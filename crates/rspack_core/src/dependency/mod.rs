mod const_dependency;
mod context_dependency;
mod context_element_dependency;
mod dependency_category;
mod dependency_id;
mod dependency_macro;
mod dependency_template;
mod dependency_trait;
mod dependency_type;
mod entry;
mod import_dependency_trait;
mod module_dependency;
mod runtime_requirements_dependency;
mod runtime_template;
mod span;
mod static_exports_dependency;

use std::sync::Arc;

pub use const_dependency::ConstDependency;
pub use context_dependency::{AsContextDependency, ContextDependency};
pub use context_element_dependency::ContextElementDependency;
pub use dependency_category::DependencyCategory;
pub use dependency_id::*;
pub use dependency_template::*;
pub use dependency_trait::*;
pub use dependency_type::DependencyType;
pub use entry::*;
pub use import_dependency_trait::ImportDependencyTrait;
pub use module_dependency::*;
pub use runtime_requirements_dependency::RuntimeRequirementsDependency;
pub use runtime_template::*;
pub use span::SpanExt;
pub use static_exports_dependency::StaticExportsDependency;
use swc_core::ecma::atoms::JsWord;

use crate::{
  ConnectionState, ModuleGraph, ModuleGraphConnection, ModuleIdentifier, ReferencedExport,
  RuntimeSpec,
};

#[derive(Debug, Default)]
pub struct ExportSpec {
  pub name: JsWord,
  pub export: Option<Nullable<Vec<JsWord>>>,
  pub exports: Option<Vec<ExportNameOrSpec>>,
  pub can_mangle: Option<bool>,
  pub terminal_binding: Option<bool>,
  pub priority: Option<u8>,
  pub hidden: Option<bool>,
  pub from: Option<ModuleGraphConnection>,
  pub from_export: Option<ModuleGraphConnection>,
}

#[derive(Debug)]
pub enum Nullable<T> {
  Null,
  Value(T),
}

impl ExportSpec {
  pub fn new(name: String) -> Self {
    Self {
      name: JsWord::from(name),
      ..Default::default()
    }
  }
}

#[derive(Debug)]
pub enum ExportNameOrSpec {
  String(JsWord),
  ExportSpec(ExportSpec),
}

impl Default for ExportNameOrSpec {
  fn default() -> Self {
    Self::String(JsWord::default())
  }
}

#[derive(Debug, Default)]
pub enum ExportsOfExportsSpec {
  True,
  #[default]
  Null,
  Array(Vec<ExportNameOrSpec>),
}

#[derive(Debug, Default)]
#[allow(unused)]
pub struct ExportsSpec {
  pub exports: ExportsOfExportsSpec,
  pub priority: Option<u8>,
  pub can_mangle: Option<bool>,
  pub terminal_binding: Option<bool>,
  pub from: Option<ModuleGraphConnection>,
  pub dependencies: Option<Vec<ModuleIdentifier>>,
  pub hide_export: Option<Vec<JsWord>>,
  pub exclude_exports: Option<Vec<JsWord>>,
}

pub enum ExportsReferencedType {
  No,     // NO_EXPORTS_REFERENCED
  Object, // EXPORTS_OBJECT_REFERENCED
  String(Box<Vec<Vec<JsWord>>>),
  Value(Box<Vec<ReferencedExport>>),
}

impl From<JsWord> for ExportsReferencedType {
  fn from(value: JsWord) -> Self {
    ExportsReferencedType::String(Box::new(vec![vec![value]]))
  }
}

impl From<Vec<Vec<JsWord>>> for ExportsReferencedType {
  fn from(value: Vec<Vec<JsWord>>) -> Self {
    ExportsReferencedType::String(Box::new(value))
  }
}

impl From<Vec<JsWord>> for ExportsReferencedType {
  fn from(value: Vec<JsWord>) -> Self {
    ExportsReferencedType::String(Box::new(vec![value]))
  }
}

impl From<Vec<ReferencedExport>> for ExportsReferencedType {
  fn from(value: Vec<ReferencedExport>) -> Self {
    ExportsReferencedType::Value(Box::new(value))
  }
}

pub type DependencyConditionFn = Arc<
  dyn Fn(&ModuleGraphConnection, Option<&RuntimeSpec>, &ModuleGraph) -> ConnectionState
    + Send
    + Sync,
>;

#[derive(Clone)]
pub enum DependencyCondition {
  False,
  Fn(DependencyConditionFn),
}

impl std::fmt::Debug for DependencyCondition {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // Self::Nil => write!(f, "Nil"),
      Self::False => write!(f, "False"),
      Self::Fn(_) => write!(f, "Fn"),
    }
  }
}

// TODO: should move to rspack_plugin_javascript once we drop old treeshaking
pub mod needs_refactor {
  use once_cell::sync::Lazy;
  use regex::Regex;
  use swc_core::{
    common::{EqIgnoreSpan, Spanned, SyntaxContext, DUMMY_SP},
    ecma::{
      ast::{
        Expr, ExprOrSpread, Id, Ident, ImportDecl, Lit, MemberExpr, MemberProp, MetaPropExpr,
        MetaPropKind, ModuleExportName, NewExpr,
      },
      atoms::JsWord,
      visit::Visit,
    },
  };

  use crate::SpanExt;

  pub fn match_new_url(new_expr: &NewExpr) -> Option<(u32, u32, String)> {
    fn is_import_meta_url(expr: &Expr) -> bool {
      static IMPORT_META: Lazy<Expr> = Lazy::new(|| {
        Expr::Member(MemberExpr {
          span: DUMMY_SP,
          obj: Box::new(Expr::MetaProp(MetaPropExpr {
            span: DUMMY_SP,
            kind: MetaPropKind::ImportMeta,
          })),
          prop: MemberProp::Ident(Ident {
            span: DUMMY_SP,
            sym: "url".into(),
            optional: false,
          }),
        })
      });
      Ident::within_ignored_ctxt(|| expr.eq_ignore_span(&IMPORT_META))
    }

    if matches!(&*new_expr.callee, Expr::Ident(Ident { sym, .. }) if sym == "URL")
      && let Some(args) = &new_expr.args
      && let (Some(first), Some(second)) = (args.first(), args.get(1))
      && let (
        ExprOrSpread {
          spread: None,
          expr: box Expr::Lit(Lit::Str(path)),
        },
        ExprOrSpread {
          spread: None,
          expr: box expr,
        },
      ) = (first, second)
      && is_import_meta_url(expr)
    {
      return Some((
        path.span.real_lo(),
        expr.span().real_hi(),
        path.value.to_string(),
      ));
    }
    None
  }

  static WORKER_FROM_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(.+?)(\(\))?\s+from\s+(.+)$").expect("invalid regex"));

  #[derive(Debug, Default)]
  pub struct WorkerSyntaxList {
    variables: Vec<WorkerSyntax>,
    globals: Vec<WorkerSyntax>,
  }

  impl WorkerSyntaxList {
    pub fn push(&mut self, syntax: WorkerSyntax) {
      if syntax.ctxt.is_some() {
        self.variables.push(syntax);
      } else {
        self.globals.push(syntax);
      }
    }

    fn find_worker_syntax(&self, ident: &Ident) -> Option<&WorkerSyntax> {
      (self.variables.iter().chain(self.globals.iter())).find(|s| s.matches(ident))
    }

    pub fn match_new_worker(&self, new_expr: &NewExpr) -> bool {
      matches!(&*new_expr.callee, Expr::Ident(ident) if self.find_worker_syntax(ident).is_some())
    }
  }

  impl Extend<WorkerSyntax> for WorkerSyntaxList {
    fn extend<T: IntoIterator<Item = WorkerSyntax>>(&mut self, iter: T) {
      for i in iter {
        self.push(i);
      }
    }
  }

  impl From<WorkerSyntaxScanner<'_>> for WorkerSyntaxList {
    fn from(value: WorkerSyntaxScanner) -> Self {
      value.result
    }
  }

  #[derive(Debug, PartialEq, Eq)]
  pub struct WorkerSyntax {
    word: JsWord,
    ctxt: Option<SyntaxContext>,
  }

  impl WorkerSyntax {
    pub fn new(word: JsWord, ctxt: Option<SyntaxContext>) -> Self {
      Self { word, ctxt }
    }

    pub fn matches(&self, ident: &Ident) -> bool {
      if let Some(ctxt) = self.ctxt {
        let (word, id_ctxt) = ident.to_id();
        word == self.word && id_ctxt == ctxt
      } else {
        self.word == ident.sym
      }
    }
  }

  pub struct WorkerSyntaxScanner<'a> {
    result: WorkerSyntaxList,
    caps: Vec<(&'a str, &'a str)>,
  }

  pub const DEFAULT_WORKER_SYNTAX: &[&str] =
    &["Worker", "SharedWorker", "Worker from worker_threads"];

  impl<'a> WorkerSyntaxScanner<'a> {
    pub fn new(syntax: &'a [&'a str]) -> Self {
      let mut result = WorkerSyntaxList::default();
      let mut caps = Vec::new();
      for s in syntax {
        if let Some(captures) = WORKER_FROM_REGEX.captures(s)
          && let Some(ids) = captures.get(1)
          && let Some(source) = captures.get(3)
        {
          caps.push((ids.as_str(), source.as_str()));
        } else {
          result.push(WorkerSyntax::new(JsWord::from(*s), None))
        }
      }
      Self { result, caps }
    }
  }

  impl Visit for WorkerSyntaxScanner<'_> {
    fn visit_import_decl(&mut self, decl: &ImportDecl) {
      let source = &*decl.src.value;
      let found = self
        .caps
        .iter()
        .filter(|cap| cap.1 == source)
        .flat_map(|cap| {
          if cap.0 == "default" {
            decl
              .specifiers
              .iter()
              .filter_map(|spec| spec.as_default())
              .map(|spec| spec.local.to_id())
              .collect::<Vec<Id>>()
          } else {
            decl
              .specifiers
              .iter()
              .filter_map(|spec| {
                spec.as_named().filter(|named| {
                  if let Some(imported) = &named.imported {
                    let s = match imported {
                      ModuleExportName::Ident(s) => &s.sym,
                      ModuleExportName::Str(s) => &s.value,
                    };
                    s == cap.0
                  } else {
                    &*named.local.sym == cap.0
                  }
                })
              })
              .map(|spec| spec.local.to_id())
              .collect::<Vec<Id>>()
          }
        })
        .map(|pair| WorkerSyntax::new(pair.0, Some(pair.1)));
      self.result.extend(found);
    }
  }
}
