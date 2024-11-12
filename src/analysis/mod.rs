use crate::{utils::text_mod::TextMod, CliArgs};
use std::{cell::Cell, time::Duration};

// use rustc_index::Idx;
use rustc_middle::mir;
use rustc_middle::mir::visit::Visitor;
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_index::IndexVec;

pub struct Analyzer<'tcx> {
    tcx: ty::TyCtxt<'tcx>,
    cli_args: CliArgs,
}

impl<'tcx> Analyzer<'tcx> {
    pub fn new(tcx: ty::TyCtxt<'tcx>, cli_args: CliArgs) -> Self {
        Self { tcx, cli_args }
    }

    fn pre_process_cli_args(&self) {
        log::debug!("Pre-processing CLI arguments");
        if self.cli_args.print_crate {
            log::debug!("Printing the crate");
            let resolver_and_krate = self.tcx.resolver_for_lowering().borrow();
            let krate = &*resolver_and_krate.1;
            println!("{:#?}", krate);
        }

        // In case of "optimized" MIR, in the `config` callback we do not set the `mir_opt_level` to 0.
        if self.cli_args.print_mir || self.cli_args.print_unoptimized_mir {
            log::debug!("Printing the MIR");
            mir::write_mir_pretty(self.tcx, None, &mut std::io::stdout())
                .expect("write_mir_pretty failed");
        }
    }

    fn post_process_cli_args(&self) {
        log::debug!("Post-processing CLI arguments");
    }

    fn modify_if_needed(&self, msg: &str, text_mod: TextMod) -> String {
        if self.cli_args.color_log {
            text_mod.apply(msg)
        } else {
            msg.to_string()
        }
    }

    fn run_analysis(&mut self, name: &str, f: impl FnOnce(&Self)) {
        log::debug!("Running analysis: {}", name);
        f(self);
        log::debug!("Finished analysis: {}", name);
    }

    pub fn run(&mut self) {
        self.pre_process_cli_args();
        self.run_analysis("FirstAnalysis", |analyzer| {
            FirstAnalysis::new(analyzer).run();
        });
        self.post_process_cli_args();
    }
}

struct FirstAnalysis<'tcx, 'a> {
    analyzer: &'a Analyzer<'tcx>,
    elapsed: Cell<Option<Duration>>,
}

impl<'tcx, 'a> FirstAnalysis<'tcx, 'a> {
    pub fn new(analyzer: &'a Analyzer<'tcx>) -> Self {
        Self {
            analyzer,
            elapsed: Cell::new(None),
        }
    }

    fn visitor(&self) {
        let visitor = &mut FirstVisitor {
            analyzer: self.analyzer,
            stack_local_def_id: Vec::new(),
        };

        // We do not need to call `mir_keys` (self.analyzer.tcx.mir_keys(()))
        // because it returns also the enum and struct constructors
        // automatically generated by the compiler.
        //
        // For example, for the following code
        // ```no_run
        // struct MyStruct(i32);
        // enum MyEnum { Variant(i32) }
        // ```
        // the `mir_keys` returns the following local_def_ids
        // ```no_run
        // MyStruct::{constructor#0})
        // MyEnum::Variant::{constructor#0})
        // ```
        for local_def_id in self.analyzer.tcx.hir().body_owners() {
            // Visit the body of the `local_def_id`
            visitor.start_visit(
                local_def_id,
                self.analyzer
                    .tcx
                    .instance_mir(ty::InstanceKind::Item(local_def_id.to_def_id())),
            );

            // TODO: Check if the body has some promoted MIR.
            // It is not clear if analyzing the promoted MIR is necessary.
            let _promoted_mir = self.analyzer.tcx.promoted_mir(local_def_id.to_def_id());

            // let stmts = &body.basic_blocks[BasicBlock::new(0)].statements;
            // let terminator = &body.basic_blocks[BasicBlock::new(0)].terminator;
            // if !stmts.is_empty() {
            //     let first = &stmts[0].kind;
            //     match first {
            //         mir::StatementKind::Assign(bbox) => {
            //             // let place = &bbox.0;
            //             let rvalue = &bbox.1;
            //             match rvalue {
            //                 mir::Rvalue::Use(operand) => {
            //                     let op = &operand;
            //                     match op {
            //                         mir::Operand::Copy(place) => {
            //                             println!("{:#?}", place);
            //                         }
            //                         mir::Operand::Move(place) => {
            //                             println!("{:#?}", place);
            //                         }
            //                         mir::Operand::Constant(constant) => {
            //                             println!("hello {:#?}", constant);
            //                         }
            //                     }
            //                 }
            //                 _ => println!(),
            //             }
            //         }
            //         _ => println!(),
            //     }
            // }

            // if let Some(t) = terminator {
            //     let kind = &t.kind;
            //     match kind {
            //         mir::TerminatorKind::Call {
            //             func,
            //             args,
            //             destination,
            //             ..
            //         } => {
            //             println!("{:#?}", func);
            //             println!("{:#?}", args);
            //             println!("{:#?}", destination);
            //         }
            //         _ => println!(),
            //     }
            // }
            // println!();

            // println!("{:#?}\n", body);
            // println!("{:#?}\n", promoted_mir);
            // println!("{:#?}\n", body.local_decls);
            // println!("{:#?}\n", body.basic_blocks);
        }
    }

    pub fn run(&self) {
        let start_time = std::time::Instant::now();
        self.visitor();
        let elapsed = start_time.elapsed();
        self.elapsed.set(Some(elapsed));
    }
}

struct FirstVisitor<'tcx, 'a> {
    analyzer: &'a Analyzer<'tcx>,

    // Current stack of local_def_id and local_decls
    stack_local_def_id: Vec<(LocalDefId, &'a IndexVec<mir::Local, mir::LocalDecl<'tcx>>)>,
}

// Guardare le tre diverse tipologie di linear: copy move e borrow
impl<'tcx, 'a> FirstVisitor<'tcx, 'a> {
    fn start_visit(
        &mut self,
        local_def_id: LocalDefId,
        body: &'a mir::Body<'tcx>,
    ) {
        log::debug!("Visiting the local_def_id: {:?}", local_def_id);
        self.stack_local_def_id.push((local_def_id, &body.local_decls));
        self.visit_body(body);
        self.stack_local_def_id.pop();
    }
}

impl<'tcx> Visitor<'tcx> for FirstVisitor<'tcx, '_> {
    // Entry point
    fn visit_body(&mut self, body: &mir::Body<'tcx>) {
        log::trace!("Visiting the body {:?}", body);
        self.super_body(body);
    }

    // Call by the super_body
    fn visit_ty(&mut self, ty: ty::Ty<'tcx>, context: mir::visit::TyContext) {
        log::trace!("Visiting the ty: {:?}, {:?}", ty, context);
        // TODO: We should visit the `FnDef` because in `_12 = test_own(move _13) -> [return: bb5, unwind continue];`
        // `test_own` is a `FnDef`.
        self.super_ty(ty);
    }

    // Call by the super_body
    fn visit_basic_block_data(&mut self, block: mir::BasicBlock, data: &mir::BasicBlockData<'tcx>) {
        log::trace!("Visiting the basic block data: {:?}, {:?}", block, data);
        self.super_basic_block_data(block, data);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_source_scope(&mut self, scope: mir::SourceScope) {
        self.super_source_scope(scope);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_local_decl(&mut self, local: mir::Local, local_decl: &mir::LocalDecl<'tcx>) {
        self.super_local_decl(local, local_decl);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_user_type_annotation(
        &mut self,
        index: ty::UserTypeAnnotationIndex,
        ty: &ty::CanonicalUserTypeAnnotation<'tcx>,
    ) {
        self.super_user_type_annotation(index, ty);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_var_debug_info(&mut self, var_debug_info: &mir::VarDebugInfo<'tcx>) {
        self.super_var_debug_info(var_debug_info);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_span(&mut self, span: rustc_span::Span) {
        self.super_span(span);
    }

    // TODO: implement
    // Call by the super_body
    fn visit_const_operand(&mut self, constant: &mir::ConstOperand<'tcx>, location: mir::Location) {
        self.super_const_operand(constant, location);
    }

    // Call by the super_basic_block_data
    fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: mir::Location) {
        log::trace!("Visiting the statement: {:?}, {:?}", statement, location);
        self.super_statement(statement, location)
    }

    // Call by the super_basic_block_data
    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: mir::Location) {
        let message = self.analyzer.modify_if_needed(
            format!("Visiting the terminator: {:?}, {:?}", terminator, location).as_str(),
            TextMod::Green,
        );
        log::trace!("{}", message);
        self.super_terminator(terminator, location)
    }

    // Call by the super_statement
    fn visit_source_info(&mut self, source_info: &mir::SourceInfo) {
        log::trace!("Visiting the source info: {:?}", source_info);
        self.super_source_info(source_info)
    }

    // Call by super_statement
    fn visit_assign(
        &mut self,
        place: &mir::Place<'tcx>,
        rvalue: &mir::Rvalue<'tcx>,
        location: mir::Location,
    ) {
        let message = self.analyzer.modify_if_needed(
            format!(
                "Visiting the assign: {:?}, {:?}, {:?}",
                place, rvalue, location
            )
            .as_str(),
            TextMod::Green,
        );
        log::trace!("{}", message);
        self.super_assign(place, rvalue, location);
    }

    // TODO: Add the other from super_statement

    // Call by the super_assign
    fn visit_place(
        &mut self,
        place: &mir::Place<'tcx>,
        context: mir::visit::PlaceContext,
        location: mir::Location,
    ) {
        log::trace!(
            "Visiting the place: {:?}, {:?}, {:?}",
            place,
            context,
            location
        );
        self.super_place(place, context, location);
    }

    // Call by the super_assign
    fn visit_rvalue(&mut self, rvalue: &mir::Rvalue<'tcx>, location: mir::Location) {
        log::trace!("Visiting the rvalue: {:?}, {:?}", rvalue, location);
        match rvalue {
            mir::Rvalue::Use(operand) => log::trace!("Operand: {:?}", operand),
            mir::Rvalue::Repeat(operand, _) => log::trace!("Operand: {:?}", operand),
            mir::Rvalue::Ref(region, borrow_kind, place) => log::trace!(
                "Region: {:?}, BorrowKind: {:?}, Place: {:?}",
                region,
                borrow_kind,
                place
            ),
            mir::Rvalue::ThreadLocalRef(def_id) => log::trace!("DefId: {:?}", def_id),
            mir::Rvalue::RawPtr(mutability, place) => {
                log::trace!("Mutability: {:?}, Place: {:?}", mutability, place)
            }
            mir::Rvalue::Len(place) => log::trace!("Place: {:?}", place),
            mir::Rvalue::Cast(cast_kind, operand, ty) => log::trace!(
                "CastKind: {:?}, Operand: {:?}, Ty: {:?}",
                cast_kind,
                operand,
                ty
            ),
            mir::Rvalue::BinaryOp(bin_op, _) => log::trace!("BinOp: {:?}", bin_op),
            mir::Rvalue::NullaryOp(null_op, ty) => {
                log::trace!("NullOp: {:?}, Ty: {:?}", null_op, ty)
            }
            mir::Rvalue::UnaryOp(un_op, operand) => {
                log::trace!("UnOp: {:?}, Operand: {:?}", un_op, operand)
            }
            mir::Rvalue::Discriminant(place) => log::trace!("Place: {:?}", place),
            mir::Rvalue::Aggregate(aggregate_kind, index_vec) => log::trace!(
                "AggregateKind: {:?}, IndexVec: {:?}",
                aggregate_kind,
                index_vec
            ),
            mir::Rvalue::ShallowInitBox(operand, ty) => {
                log::trace!("Operand: {:?}, Ty: {:?}", operand, ty)
            }
            mir::Rvalue::CopyForDeref(place) => log::trace!("Place: {:?}", place),
        }
    }
}
