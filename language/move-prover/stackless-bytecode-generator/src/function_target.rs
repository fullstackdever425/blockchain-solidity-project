// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    annotations::Annotations,
    lifetime_analysis, reaching_def_analysis,
    stackless_bytecode::{AttrId, Bytecode},
};
use itertools::Itertools;
use spec_lang::{
    ast::Condition,
    env::{FunId, FunctionEnv, GlobalEnv, Loc, TypeParameter},
    symbol::{Symbol, SymbolPool},
    ty::{Type, TypeDisplayContext},
};
use std::{cell::RefCell, collections::BTreeMap, fmt};
use vm::file_format::CodeOffset;

/// A FunctionTarget is a drop-in replacement for a FunctionEnv which allows to rewrite
/// and analyze bytecode and parameter/local types. It encapsulates a FunctionEnv and information
/// which can be rewritten using the `FunctionTargetsHolder` data structure.
pub struct FunctionTarget<'env> {
    pub func_env: &'env FunctionEnv<'env>,
    pub data: &'env FunctionTargetData,

    // Used for debugging and testing, containing any attached annotation formatters.
    annotation_formatters: RefCell<Vec<Box<AnnotationFormatter>>>,
}

/// Holds the owned data belonging to a FunctionTarget, which can be rewritten using
/// the `FunctionTargetsHolder::rewrite` method.
#[derive(Debug)]
pub struct FunctionTargetData {
    pub code: Vec<Bytecode>,
    pub local_types: Vec<Type>,
    pub return_types: Vec<Type>,
    pub locations: BTreeMap<AttrId, Loc>,
    pub annotations: Annotations,
}

impl<'env> FunctionTarget<'env> {
    pub fn new(
        func_env: &'env FunctionEnv<'env>,
        data: &'env FunctionTargetData,
    ) -> FunctionTarget<'env> {
        FunctionTarget {
            func_env,
            data,
            annotation_formatters: RefCell::new(vec![]),
        }
    }

    /// Returns the name of this function.
    pub fn get_name(&self) -> Symbol {
        self.func_env.get_name()
    }

    /// Gets the id of this function.
    pub fn get_id(&self) -> FunId {
        self.func_env.get_id()
    }

    /// Shortcut for accessing the symbol pool.
    pub fn symbol_pool(&self) -> &SymbolPool {
        self.func_env.module_env.symbol_pool()
    }

    /// Shortcut for accessing the global env of this function.
    pub fn global_env(&self) -> &GlobalEnv {
        self.func_env.module_env.env
    }

    /// Returns the location of this function.
    pub fn get_loc(&self) -> Loc {
        self.func_env.get_loc()
    }

    /// Returns the location of the bytecode at the given offset.
    pub fn get_bytecode_loc(&self, attr_id: AttrId) -> Loc {
        if let Some(loc) = self.data.locations.get(&attr_id) {
            loc.clone()
        } else {
            self.get_loc()
        }
    }

    /// Returns true if this function is native.
    pub fn is_native(&self) -> bool {
        self.func_env.is_native()
    }

    /// Returns true if this function is public.
    pub fn is_public(&self) -> bool {
        self.func_env.is_public()
    }

    /// Returns true if this function mutates any references (i.e. has &mut parameters).
    pub fn is_mutating(&self) -> bool {
        self.func_env.is_mutating()
    }

    /// Returns the type parameters associated with this function.
    pub fn get_type_parameters(&self) -> Vec<TypeParameter> {
        self.func_env.get_type_parameters()
    }

    /// Returns return type at given index.
    pub fn get_return_type(&self, idx: usize) -> &Type {
        &self.data.return_types[idx]
    }

    /// Returns return types of this function.
    pub fn get_return_types(&self) -> &[Type] {
        &self.data.return_types
    }

    /// Returns the number of return values of this function.
    pub fn get_return_count(&self) -> usize {
        self.data.return_types.len()
    }

    pub fn get_parameter_count(&self) -> usize {
        self.func_env.get_parameter_count()
    }

    /// Get the name to be used for a local. If the local is an argument, use that for naming,
    /// otherwise generate a unique name.
    pub fn get_local_name(&self, idx: usize) -> Symbol {
        self.func_env.get_local_name(idx)
    }

    /// Gets the number of locals of this function, including parameters.
    pub fn get_local_count(&self) -> usize {
        self.data.local_types.len()
    }

    /// Gets the number of user declared locals of this function, excluding locals which have
    /// been introduced by transformations.
    pub fn get_user_local_count(&self) -> usize {
        self.func_env.get_local_count()
    }

    /// Gets the type of the local at index. This must use an index in the range as determined by
    /// `get_local_count`.
    pub fn get_local_type(&self, idx: usize) -> &Type {
        &self.data.local_types[idx]
    }

    /// Returns specification conditions associated with this function.
    pub fn get_specification_on_decl(&'env self) -> &'env [Condition] {
        self.func_env.get_specification_on_decl()
    }

    /// Returns specification conditions associated with this function at bytecode offset.
    pub fn get_specification_on_impl(&'env self, offset: CodeOffset) -> Option<&'env [Condition]> {
        self.func_env.get_specification_on_impl(offset)
    }

    /// Gets the bytecode.
    pub fn get_code(&self) -> &[Bytecode] {
        &self.data.code
    }

    /// Gets annotations.
    pub fn get_annotations(&self) -> &Annotations {
        &self.data.annotations
    }
}

// =================================================================================================
// Formatting

/// A function which is called to display the value of an annotation for a given function target
/// at the given code offset. The function is passed the function target and the code offset, and
/// is expected to pick the annotation of its respective type from the function target and for
/// the given code offset. It should return None if there is no relevant annotation.
pub type AnnotationFormatter = dyn Fn(&FunctionTarget<'_>, CodeOffset) -> Option<String>;

impl<'env> FunctionTarget<'env> {
    /// Register a formatter. Each function target processor which introduces new annotations
    /// should register a formatter in order to get is value printed when a function target
    /// is displayed for debugging or testing.
    pub fn register_annotation_formatter(&self, formatter: Box<AnnotationFormatter>) {
        self.annotation_formatters.borrow_mut().push(formatter);
    }

    /// Tests use this function to register all relevant annotation formatters. Extend this with
    /// new formatters relevant for tests.
    pub fn register_annotation_formatters_for_test(&self) {
        self.register_annotation_formatter(Box::new(lifetime_analysis::format_lifetime_annotation));
        self.register_annotation_formatter(Box::new(
            reaching_def_analysis::format_reaching_def_annotation,
        ));
    }
}

impl<'env> fmt::Display for FunctionTarget<'env> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}fun {}::{}",
            if self.is_public() { "pub " } else { "" },
            self.func_env
                .module_env
                .get_name()
                .display(self.symbol_pool()),
            self.get_name().display(self.symbol_pool())
        )?;
        let tparams = &self.get_type_parameters();
        if !tparams.is_empty() {
            write!(f, "<")?;
            for (i, TypeParameter(name, _)) in tparams.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", name.display(self.symbol_pool()))?;
            }
            write!(f, ">")?;
        }
        let tctx = TypeDisplayContext::WithEnv {
            env: self.global_env(),
        };
        write!(f, "(")?;
        for i in 0..self.get_parameter_count() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(
                f,
                "{}: {}",
                self.get_local_name(i).display(self.symbol_pool()),
                self.get_local_type(i).display(&tctx)
            )?;
        }
        write!(f, ")")?;
        if self.get_return_count() > 0 {
            write!(f, ": ")?;
            if self.get_return_count() > 1 {
                write!(f, "(")?;
            }
            for i in 0..self.get_return_count() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", self.get_return_type(i).display(&tctx))?;
            }
            if self.get_return_count() > 1 {
                write!(f, ")")?;
            }
        }
        writeln!(f, " {{")?;
        for i in self.get_parameter_count()..self.get_local_count() {
            writeln!(
                f,
                "    var {}: {}",
                self.get_local_name(i).display(self.symbol_pool()),
                self.get_local_type(i).display(&tctx)
            )?;
        }
        for (offset, code) in self.get_code().iter().enumerate() {
            let annotations = self
                .annotation_formatters
                .borrow()
                .iter()
                .filter_map(|f| f(self, offset as CodeOffset))
                .join(", ");
            if !annotations.is_empty() {
                writeln!(f, "    // {}", annotations)?;
            }
            writeln!(f, "    {}", code.display(self))?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}
