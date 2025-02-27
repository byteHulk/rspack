use rspack_core::{
  AsContextDependency, AsModuleDependency, Dependency, DependencyCategory, DependencyId,
  DependencyTemplate, DependencyType, ExportNameOrSpec, ExportsOfExportsSpec, ExportsSpec,
  HarmonyExportInitFragment, ModuleGraph, TemplateContext, TemplateReplaceSource, UsedName,
};
use swc_core::ecma::atoms::JsWord;

// Create _webpack_require__.d(__webpack_exports__, {}) for each export.
#[derive(Debug, Clone)]
pub struct HarmonyExportSpecifierDependency {
  id: DependencyId,
  name: JsWord,
  value: JsWord, // id
}

impl HarmonyExportSpecifierDependency {
  pub fn new(name: JsWord, value: JsWord) -> Self {
    Self {
      id: DependencyId::new(),
      name,
      value,
    }
  }
}

impl Dependency for HarmonyExportSpecifierDependency {
  fn dependency_debug_name(&self) -> &'static str {
    "HarmonyExportSpecifierDependency"
  }

  fn id(&self) -> &DependencyId {
    &self.id
  }

  fn category(&self) -> &DependencyCategory {
    &DependencyCategory::Esm
  }

  fn dependency_type(&self) -> &DependencyType {
    &DependencyType::EsmExportSpecifier
  }

  fn get_exports(&self, _mg: &ModuleGraph) -> Option<ExportsSpec> {
    Some(ExportsSpec {
      exports: ExportsOfExportsSpec::Array(vec![ExportNameOrSpec::String(self.name.clone())]),
      priority: Some(1),
      can_mangle: None,
      terminal_binding: Some(true),
      from: None,
      dependencies: None,
      hide_export: None,
      exclude_exports: None,
    })
  }

  fn get_module_evaluation_side_effects_state(
    &self,
    _module_graph: &rspack_core::ModuleGraph,
    _module_chain: &mut rustc_hash::FxHashSet<rspack_core::ModuleIdentifier>,
  ) -> rspack_core::ConnectionState {
    rspack_core::ConnectionState::Bool(false)
  }
}

impl AsModuleDependency for HarmonyExportSpecifierDependency {}

impl DependencyTemplate for HarmonyExportSpecifierDependency {
  fn apply(
    &self,
    _source: &mut TemplateReplaceSource,
    code_generatable_context: &mut TemplateContext,
  ) {
    let TemplateContext {
      init_fragments,
      compilation,
      module,
      runtime,
      ..
    } = code_generatable_context;

    let module = compilation
      .module_graph
      .module_by_identifier(&module.identifier())
      .expect("should have module graph module");

    let used_name = if compilation.options.is_new_tree_shaking() {
      let exports_info_id = compilation
        .module_graph
        .get_exports_info(&module.identifier())
        .id;
      let used_name = exports_info_id.get_used_name(
        &compilation.module_graph,
        *runtime,
        UsedName::Str(self.name.clone()),
      );
      used_name.map(|item| match item {
        UsedName::Str(name) => name,
        UsedName::Vec(vec) => {
          // vec.contains(&self.name)
          // TODO: should align webpack
          vec[0].clone()
        }
      })
    } else if compilation.options.builtins.tree_shaking.is_true() {
      compilation
        .module_graph
        .get_exports_info(&module.identifier())
        .old_get_used_exports()
        .contains(&self.name)
        .then(|| self.name.clone())
    } else {
      Some(self.name.clone())
    };
    if let Some(used_name) = used_name {
      init_fragments.push(Box::new(HarmonyExportInitFragment::new(
        module.get_exports_argument(),
        vec![(used_name, self.value.clone())],
      )));
    }
  }
}

impl AsContextDependency for HarmonyExportSpecifierDependency {}
