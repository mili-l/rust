use crate::utils::{match_def_path, paths, span_lint_and_note};
use rustc_hir::def_id::DefId;
use rustc_hir::{AsyncGeneratorKind, Body, BodyId, GeneratorKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::GeneratorInteriorTypeCause;
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::Span;

declare_clippy_lint! {
    /// **What it does:** Checks for calls to await while holding a
    /// `RefCell` `Ref` or `RefMut`.
    ///
    /// **Why is this bad?** `RefCell` refs only check for exclusive mutable access
    /// at runtime. Holding onto a `RefCell` ref across an `await` suspension point
    /// risks panics from a mutable ref shared while other refs are outstanding.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    ///
    /// ```rust,ignore
    /// use std::cell::RefCell;
    ///
    /// async fn foo(x: &RefCell<u32>) {
    ///   let b = x.borrow_mut()();
    ///   *ref += 1;
    ///   bar.await;
    /// }
    /// ```
    ///
    /// Use instead:
    /// ```rust,ignore
    /// use std::cell::RefCell;
    ///
    /// async fn foo(x: &RefCell<u32>) {
    ///   {
    ///     let b = x.borrow_mut();
    ///     *ref += 1;
    ///   }
    ///   bar.await;
    /// }
    /// ```
    pub AWAIT_HOLDING_REFCELL_REF,
    pedantic,
    "Inside an async function, holding a RefCell ref while calling await"
}

declare_lint_pass!(AwaitHoldingRefCellRef => [AWAIT_HOLDING_REFCELL_REF]);

impl LateLintPass<'_> for AwaitHoldingRefCellRef {
    fn check_body(&mut self, cx: &LateContext<'_>, body: &'_ Body<'_>) {
        use AsyncGeneratorKind::{Block, Closure, Fn};
        if let Some(GeneratorKind::Async(Block | Closure | Fn)) = body.generator_kind {
            let body_id = BodyId {
                hir_id: body.value.hir_id,
            };
            let def_id = cx.tcx.hir().body_owner_def_id(body_id);
            let typeck_results = cx.tcx.typeck(def_id);
            check_interior_types(cx, &typeck_results.generator_interior_types, body.value.span);
        }
    }
}

fn check_interior_types(cx: &LateContext<'_>, ty_causes: &[GeneratorInteriorTypeCause<'_>], span: Span) {
    for ty_cause in ty_causes {
        if let rustc_middle::ty::Adt(adt, _) = ty_cause.ty.kind() {
            if is_refcell_ref(cx, adt.did) {
                span_lint_and_note(
                    cx,
                    AWAIT_HOLDING_REFCELL_REF,
                    ty_cause.span,
                    "this RefCell Ref is held across an 'await' point. Consider ensuring the Ref is dropped before calling await.",
                    ty_cause.scope_span.or(Some(span)),
                    "these are all the await points this ref is held through",
                );
            }
        }
    }
}

fn is_refcell_ref(cx: &LateContext<'_>, def_id: DefId) -> bool {
    match_def_path(cx, def_id, &paths::REFCELL_REF) || match_def_path(cx, def_id, &paths::REFCELL_REFMUT)
}
