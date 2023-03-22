//! Code shared by trait and projection goals for candidate assembly.

#[cfg(doc)]
use super::trait_goals::structural_traits::*;
use super::EvalCtxt;
use itertools::Itertools;
use rustc_hir::def_id::DefId;
use rustc_infer::traits::query::NoSolution;
use rustc_infer::traits::util::elaborate_predicates;
use rustc_middle::traits::solve::{CanonicalResponse, Certainty, Goal, MaybeCause, QueryResult};
use rustc_middle::ty::fast_reject::TreatProjections;
use rustc_middle::ty::TypeFoldable;
use rustc_middle::ty::{self, Ty, TyCtxt};
use std::fmt::Debug;

/// A candidate is a possible way to prove a goal.
///
/// It consists of both the `source`, which describes how that goal would be proven,
/// and the `result` when using the given `source`.
#[derive(Debug, Clone)]
pub(super) struct Candidate<'tcx> {
    pub(super) source: CandidateSource,
    pub(super) result: CanonicalResponse<'tcx>,
}

/// Possible ways the given goal can be proven.
#[derive(Debug, Clone, Copy)]
pub(super) enum CandidateSource {
    /// A user written impl.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// fn main() {
    ///     let x: Vec<u32> = Vec::new();
    ///     // This uses the impl from the standard library to prove `Vec<T>: Clone`.
    ///     let y = x.clone();
    /// }
    /// ```
    Impl(DefId),
    /// A builtin impl generated by the compiler. When adding a new special
    /// trait, try to use actual impls whenever possible. Builtin impls should
    /// only be used in cases where the impl cannot be manually be written.
    ///
    /// Notable examples are auto traits, `Sized`, and `DiscriminantKind`.
    /// For a list of all traits with builtin impls, check out the
    /// [`EvalCtxt::assemble_builtin_impl_candidates`] method. Not
    BuiltinImpl,
    /// An assumption from the environment.
    ///
    /// More precicely we've used the `n-th` assumption in the `param_env`.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// fn is_clone<T: Clone>(x: T) -> (T, T) {
    ///     // This uses the assumption `T: Clone` from the `where`-bounds
    ///     // to prove `T: Clone`.
    ///     (x.clone(), x)
    /// }
    /// ```
    ParamEnv(usize),
    /// If the self type is an alias type, e.g. an opaque type or a projection,
    /// we know the bounds on that alias to hold even without knowing its concrete
    /// underlying type.
    ///
    /// More precisely this candidate is using the `n-th` bound in the `item_bounds` of
    /// the self type.
    ///
    /// ## Examples
    ///
    /// ```rust
    /// trait Trait {
    ///     type Assoc: Clone;
    /// }
    ///
    /// fn foo<T: Trait>(x: <T as Trait>::Assoc) {
    ///     // We prove `<T as Trait>::Assoc` by looking at the bounds on `Assoc` in
    ///     // in the trait definition.
    ///     let _y = x.clone();
    /// }
    /// ```
    AliasBound,
}

/// Methods used to assemble candidates for either trait or projection goals.
pub(super) trait GoalKind<'tcx>: TypeFoldable<TyCtxt<'tcx>> + Copy + Eq {
    fn self_ty(self) -> Ty<'tcx>;

    fn with_self_ty(self, tcx: TyCtxt<'tcx>, self_ty: Ty<'tcx>) -> Self;

    fn trait_def_id(self, tcx: TyCtxt<'tcx>) -> DefId;

    // Consider a clause, which consists of a "assumption" and some "requirements",
    // to satisfy a goal. If the requirements hold, then attempt to satisfy our
    // goal by equating it with the assumption.
    fn consider_implied_clause(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
        assumption: ty::Predicate<'tcx>,
        requirements: impl IntoIterator<Item = Goal<'tcx, ty::Predicate<'tcx>>>,
    ) -> QueryResult<'tcx>;

    // Consider a clause specifically for a `dyn Trait` self type. This requires
    // additionally checking all of the supertraits and object bounds to hold,
    // since they're not implied by the well-formedness of the object type.
    fn consider_object_bound_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
        assumption: ty::Predicate<'tcx>,
    ) -> QueryResult<'tcx>;

    fn consider_impl_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
        impl_def_id: DefId,
    ) -> QueryResult<'tcx>;

    // A type implements an `auto trait` if its components do as well. These components
    // are given by built-in rules from [`instantiate_constituent_tys_for_auto_trait`].
    fn consider_auto_trait_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A trait alias holds if the RHS traits and `where` clauses hold.
    fn consider_trait_alias_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A type is `Copy` or `Clone` if its components are `Sized`. These components
    // are given by built-in rules from [`instantiate_constituent_tys_for_sized_trait`].
    fn consider_builtin_sized_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A type is `Copy` or `Clone` if its components are `Copy` or `Clone`. These
    // components are given by built-in rules from [`instantiate_constituent_tys_for_copy_clone_trait`].
    fn consider_builtin_copy_clone_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A type is `PointerLike` if we can compute its layout, and that layout
    // matches the layout of `usize`.
    fn consider_builtin_pointer_like_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A callable type (a closure, fn def, or fn ptr) is known to implement the `Fn<A>`
    // family of traits where `A` is given by the signature of the type.
    fn consider_builtin_fn_trait_candidates(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
        kind: ty::ClosureKind,
    ) -> QueryResult<'tcx>;

    // `Tuple` is implemented if the `Self` type is a tuple.
    fn consider_builtin_tuple_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // `Pointee` is always implemented.
    //
    // See the projection implementation for the `Metadata` types for all of
    // the built-in types. For structs, the metadata type is given by the struct
    // tail.
    fn consider_builtin_pointee_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A generator (that comes from an `async` desugaring) is known to implement
    // `Future<Output = O>`, where `O` is given by the generator's return type
    // that was computed during type-checking.
    fn consider_builtin_future_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // A generator (that doesn't come from an `async` desugaring) is known to
    // implement `Generator<R, Yield = Y, Return = O>`, given the resume, yield,
    // and return types of the generator computed during type-checking.
    fn consider_builtin_generator_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // The most common forms of unsizing are array to slice, and concrete (Sized)
    // type into a `dyn Trait`. ADTs and Tuples can also have their final field
    // unsized if it's generic.
    fn consider_builtin_unsize_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;

    // `dyn Trait1` can be unsized to `dyn Trait2` if they are the same trait, or
    // if `Trait2` is a (transitive) supertrait of `Trait2`.
    fn consider_builtin_dyn_upcast_candidates(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> Vec<CanonicalResponse<'tcx>>;

    fn consider_builtin_discriminant_kind_candidate(
        ecx: &mut EvalCtxt<'_, 'tcx>,
        goal: Goal<'tcx, Self>,
    ) -> QueryResult<'tcx>;
}

impl<'tcx> EvalCtxt<'_, 'tcx> {
    pub(super) fn assemble_and_evaluate_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
    ) -> Vec<Candidate<'tcx>> {
        debug_assert_eq!(goal, self.resolve_vars_if_possible(goal));

        // HACK: `_: Trait` is ambiguous, because it may be satisfied via a builtin rule,
        // object bound, alias bound, etc. We are unable to determine this until we can at
        // least structually resolve the type one layer.
        if goal.predicate.self_ty().is_ty_var() {
            return vec![Candidate {
                source: CandidateSource::BuiltinImpl,
                result: self
                    .evaluate_added_goals_and_make_canonical_response(Certainty::AMBIGUOUS)
                    .unwrap(),
            }];
        }

        let mut candidates = Vec::new();

        self.assemble_candidates_after_normalizing_self_ty(goal, &mut candidates);

        self.assemble_impl_candidates(goal, &mut candidates);

        self.assemble_builtin_impl_candidates(goal, &mut candidates);

        self.assemble_param_env_candidates(goal, &mut candidates);

        self.assemble_alias_bound_candidates(goal, &mut candidates);

        self.assemble_object_bound_candidates(goal, &mut candidates);

        candidates
    }

    /// If the self type of a goal is a projection, computing the relevant candidates is difficult.
    ///
    /// To deal with this, we first try to normalize the self type and add the candidates for the normalized
    /// self type to the list of candidates in case that succeeds. Note that we can't just eagerly return in
    /// this case as projections as self types add
    // FIXME complete the unfinished sentence above
    fn assemble_candidates_after_normalizing_self_ty<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        let tcx = self.tcx();
        // FIXME: We also have to normalize opaque types, not sure where to best fit that in.
        let &ty::Alias(ty::Projection, projection_ty) = goal.predicate.self_ty().kind() else {
            return
        };

        self.probe(|ecx| {
            let normalized_ty = ecx.next_ty_infer();
            let normalizes_to_goal = goal.with(
                tcx,
                ty::Binder::dummy(ty::ProjectionPredicate {
                    projection_ty,
                    term: normalized_ty.into(),
                }),
            );
            ecx.add_goal(normalizes_to_goal);
            if let Ok(_) = ecx.try_evaluate_added_goals() {
                let normalized_ty = ecx.resolve_vars_if_possible(normalized_ty);

                // NOTE: Alternatively we could call `evaluate_goal` here and only have a `Normalized` candidate.
                // This doesn't work as long as we use `CandidateSource` in winnowing.
                let goal = goal.with(tcx, goal.predicate.with_self_ty(tcx, normalized_ty));
                candidates.extend(ecx.assemble_and_evaluate_candidates(goal));
            }
        });
    }

    fn assemble_impl_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        let tcx = self.tcx();
        tcx.for_each_relevant_impl_treating_projections(
            goal.predicate.trait_def_id(tcx),
            goal.predicate.self_ty(),
            TreatProjections::NextSolverLookup,
            |impl_def_id| match G::consider_impl_candidate(self, goal, impl_def_id) {
                Ok(result) => candidates
                    .push(Candidate { source: CandidateSource::Impl(impl_def_id), result }),
                Err(NoSolution) => (),
            },
        );
    }

    fn assemble_builtin_impl_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        let lang_items = self.tcx().lang_items();
        let trait_def_id = goal.predicate.trait_def_id(self.tcx());
        let result = if self.tcx().trait_is_auto(trait_def_id) {
            G::consider_auto_trait_candidate(self, goal)
        } else if self.tcx().trait_is_alias(trait_def_id) {
            G::consider_trait_alias_candidate(self, goal)
        } else if lang_items.sized_trait() == Some(trait_def_id) {
            G::consider_builtin_sized_candidate(self, goal)
        } else if lang_items.copy_trait() == Some(trait_def_id)
            || lang_items.clone_trait() == Some(trait_def_id)
        {
            G::consider_builtin_copy_clone_candidate(self, goal)
        } else if lang_items.pointer_like() == Some(trait_def_id) {
            G::consider_builtin_pointer_like_candidate(self, goal)
        } else if let Some(kind) = self.tcx().fn_trait_kind_from_def_id(trait_def_id) {
            G::consider_builtin_fn_trait_candidates(self, goal, kind)
        } else if lang_items.tuple_trait() == Some(trait_def_id) {
            G::consider_builtin_tuple_candidate(self, goal)
        } else if lang_items.pointee_trait() == Some(trait_def_id) {
            G::consider_builtin_pointee_candidate(self, goal)
        } else if lang_items.future_trait() == Some(trait_def_id) {
            G::consider_builtin_future_candidate(self, goal)
        } else if lang_items.gen_trait() == Some(trait_def_id) {
            G::consider_builtin_generator_candidate(self, goal)
        } else if lang_items.unsize_trait() == Some(trait_def_id) {
            G::consider_builtin_unsize_candidate(self, goal)
        } else if lang_items.discriminant_kind_trait() == Some(trait_def_id) {
            G::consider_builtin_discriminant_kind_candidate(self, goal)
        } else {
            Err(NoSolution)
        };

        match result {
            Ok(result) => {
                candidates.push(Candidate { source: CandidateSource::BuiltinImpl, result })
            }
            Err(NoSolution) => (),
        }

        // There may be multiple unsize candidates for a trait with several supertraits:
        // `trait Foo: Bar<A> + Bar<B>` and `dyn Foo: Unsize<dyn Bar<_>>`
        if lang_items.unsize_trait() == Some(trait_def_id) {
            for result in G::consider_builtin_dyn_upcast_candidates(self, goal) {
                candidates.push(Candidate { source: CandidateSource::BuiltinImpl, result });
            }
        }
    }

    fn assemble_param_env_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        for (i, assumption) in goal.param_env.caller_bounds().iter().enumerate() {
            match G::consider_implied_clause(self, goal, assumption, []) {
                Ok(result) => {
                    candidates.push(Candidate { source: CandidateSource::ParamEnv(i), result })
                }
                Err(NoSolution) => (),
            }
        }
    }

    fn assemble_alias_bound_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        let alias_ty = match goal.predicate.self_ty().kind() {
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Adt(_, _)
            | ty::Foreign(_)
            | ty::Str
            | ty::Array(_, _)
            | ty::Slice(_)
            | ty::RawPtr(_)
            | ty::Ref(_, _, _)
            | ty::FnDef(_, _)
            | ty::FnPtr(_)
            | ty::Dynamic(..)
            | ty::Closure(..)
            | ty::Generator(..)
            | ty::GeneratorWitness(_)
            | ty::GeneratorWitnessMIR(..)
            | ty::Never
            | ty::Tuple(_)
            | ty::Param(_)
            | ty::Placeholder(..)
            | ty::Infer(ty::IntVar(_) | ty::FloatVar(_))
            | ty::Error(_) => return,
            ty::Infer(ty::TyVar(_) | ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_))
            | ty::Bound(..) => bug!("unexpected self type for `{goal:?}`"),
            ty::Alias(_, alias_ty) => alias_ty,
        };

        for assumption in self.tcx().item_bounds(alias_ty.def_id).subst(self.tcx(), alias_ty.substs)
        {
            match G::consider_implied_clause(self, goal, assumption, []) {
                Ok(result) => {
                    candidates.push(Candidate { source: CandidateSource::AliasBound, result })
                }
                Err(NoSolution) => (),
            }
        }
    }

    fn assemble_object_bound_candidates<G: GoalKind<'tcx>>(
        &mut self,
        goal: Goal<'tcx, G>,
        candidates: &mut Vec<Candidate<'tcx>>,
    ) {
        let self_ty = goal.predicate.self_ty();
        let bounds = match *self_ty.kind() {
            ty::Bool
            | ty::Char
            | ty::Int(_)
            | ty::Uint(_)
            | ty::Float(_)
            | ty::Adt(_, _)
            | ty::Foreign(_)
            | ty::Str
            | ty::Array(_, _)
            | ty::Slice(_)
            | ty::RawPtr(_)
            | ty::Ref(_, _, _)
            | ty::FnDef(_, _)
            | ty::FnPtr(_)
            | ty::Alias(..)
            | ty::Closure(..)
            | ty::Generator(..)
            | ty::GeneratorWitness(_)
            | ty::GeneratorWitnessMIR(..)
            | ty::Never
            | ty::Tuple(_)
            | ty::Param(_)
            | ty::Placeholder(..)
            | ty::Infer(ty::IntVar(_) | ty::FloatVar(_))
            | ty::Error(_) => return,
            ty::Infer(ty::TyVar(_) | ty::FreshTy(_) | ty::FreshIntTy(_) | ty::FreshFloatTy(_))
            | ty::Bound(..) => bug!("unexpected self type for `{goal:?}`"),
            ty::Dynamic(bounds, ..) => bounds,
        };

        let tcx = self.tcx();
        for assumption in
            elaborate_predicates(tcx, bounds.iter().map(|bound| bound.with_self_ty(tcx, self_ty)))
        {
            match G::consider_object_bound_candidate(self, goal, assumption.predicate) {
                Ok(result) => {
                    candidates.push(Candidate { source: CandidateSource::BuiltinImpl, result })
                }
                Err(NoSolution) => (),
            }
        }
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub(super) fn merge_candidates_and_discard_reservation_impls(
        &mut self,
        mut candidates: Vec<Candidate<'tcx>>,
    ) -> QueryResult<'tcx> {
        match candidates.len() {
            0 => return Err(NoSolution),
            1 => return Ok(self.discard_reservation_impl(candidates.pop().unwrap()).result),
            _ => {}
        }

        if candidates.len() > 1 {
            let mut i = 0;
            'outer: while i < candidates.len() {
                for j in (0..candidates.len()).filter(|&j| i != j) {
                    if self.trait_candidate_should_be_dropped_in_favor_of(
                        &candidates[i],
                        &candidates[j],
                    ) {
                        debug!(candidate = ?candidates[i], "Dropping candidate #{}/{}", i, candidates.len());
                        candidates.swap_remove(i);
                        continue 'outer;
                    }
                }

                debug!(candidate = ?candidates[i], "Retaining candidate #{}/{}", i, candidates.len());
                i += 1;
            }

            // If there are *STILL* multiple candidates that have *different* response
            // results, give up and report ambiguity.
            if candidates.len() > 1 && !candidates.iter().map(|cand| cand.result).all_equal() {
                let certainty = if candidates.iter().all(|x| {
                    matches!(x.result.value.certainty, Certainty::Maybe(MaybeCause::Overflow))
                }) {
                    Certainty::Maybe(MaybeCause::Overflow)
                } else {
                    Certainty::AMBIGUOUS
                };
                return self.evaluate_added_goals_and_make_canonical_response(certainty);
            }
        }

        // FIXME: What if there are >1 candidates left with the same response, and one is a reservation impl?
        Ok(self.discard_reservation_impl(candidates.pop().unwrap()).result)
    }

    fn trait_candidate_should_be_dropped_in_favor_of(
        &self,
        candidate: &Candidate<'tcx>,
        other: &Candidate<'tcx>,
    ) -> bool {
        // FIXME: implement this
        match (candidate.source, other.source) {
            (CandidateSource::Impl(_), _)
            | (CandidateSource::ParamEnv(_), _)
            | (CandidateSource::AliasBound, _)
            | (CandidateSource::BuiltinImpl, _) => false,
        }
    }

    fn discard_reservation_impl(&mut self, mut candidate: Candidate<'tcx>) -> Candidate<'tcx> {
        if let CandidateSource::Impl(def_id) = candidate.source {
            if let ty::ImplPolarity::Reservation = self.tcx().impl_polarity(def_id) {
                debug!("Selected reservation impl");
                // We assemble all candidates inside of a probe so by
                // making a new canonical response here our result will
                // have no constraints.
                candidate.result = self
                    .evaluate_added_goals_and_make_canonical_response(Certainty::AMBIGUOUS)
                    .unwrap();
            }
        }

        candidate
    }
}
