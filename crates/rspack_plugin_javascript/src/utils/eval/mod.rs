mod eval_array_expr;
mod eval_binary_expr;
mod eval_cond_expr;
mod eval_lit_expr;
mod eval_new_expr;
mod eval_tpl_expr;
mod eval_unary_expr;

use bitflags::bitflags;
use rspack_core::DependencyLocation;

pub use self::eval_array_expr::eval_array_expression;
pub use self::eval_binary_expr::eval_binary_expression;
pub use self::eval_cond_expr::eval_cond_expression;
pub use self::eval_lit_expr::eval_lit_expr;
pub use self::eval_new_expr::eval_new_expression;
pub use self::eval_tpl_expr::{eval_tpl_expression, TemplateStringKind};
pub use self::eval_unary_expr::eval_unary_expression;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ty {
  Unknown,
  Undefined,
  Null,
  String,
  Number,
  Boolean,
  RegExp,
  Conditional,
  Array,
  ConstArray,
  BigInt,
  // Identifier,
  // TypeWrapped,
  TemplateString,
}

type Boolean = bool;
type Number = f64;
type Bigint = num_bigint::BigInt;
// type Array<'a> = &'a ast::ArrayLit;
type String = std::string::String;
type Regexp = (String, String); // (expr, flags)

// I really don't want there has many alloc, maybe this can be optimized after
// parse finished.
#[derive(Debug)]
pub struct BasicEvaluatedExpression {
  ty: Ty,
  range: Option<DependencyLocation>,
  falsy: bool,
  truthy: bool,
  side_effects: bool,
  nullish: Option<bool>,
  boolean: Option<Boolean>,
  number: Option<Number>,
  string: Option<String>,
  bigint: Option<Bigint>,
  regexp: Option<Regexp>,
  items: Option<Vec<BasicEvaluatedExpression>>,
  quasis: Option<Vec<BasicEvaluatedExpression>>,
  parts: Option<Vec<BasicEvaluatedExpression>>,
  // array: Option<Array>
  template_string_kind: Option<TemplateStringKind>,

  options: Option<Vec<BasicEvaluatedExpression>>,
}

impl Default for BasicEvaluatedExpression {
  fn default() -> Self {
    Self::new()
  }
}

impl BasicEvaluatedExpression {
  pub fn new() -> Self {
    Self {
      ty: Ty::Unknown,
      range: None,
      falsy: false,
      truthy: false,
      side_effects: true,
      nullish: None,
      boolean: None,
      number: None,
      bigint: None,
      quasis: None,
      parts: None,
      template_string_kind: None,
      options: None,
      string: None,
      items: None,
      regexp: None,
    }
  }

  pub fn with_range(start: u32, end: u32) -> Self {
    let mut expr = BasicEvaluatedExpression::new();
    expr.set_range(start, end);
    expr
  }

  // pub fn is_unknown(&self) -> bool {
  //   matches!(self.ty, Ty::Unknown)
  // }

  // pub fn is_identifier(&self) -> bool {
  //   matches!(self.ty, Ty::Identifier)
  // }

  pub fn is_null(&self) -> bool {
    matches!(self.ty, Ty::Null)
  }

  pub fn is_undefined(&self) -> bool {
    matches!(self.ty, Ty::Undefined)
  }

  pub fn is_conditional(&self) -> bool {
    matches!(self.ty, Ty::Conditional)
  }

  pub fn is_string(&self) -> bool {
    matches!(self.ty, Ty::String)
  }

  pub fn is_bool(&self) -> bool {
    matches!(self.ty, Ty::Boolean)
  }

  pub fn is_array(&self) -> bool {
    matches!(self.ty, Ty::Array)
  }

  pub fn is_template_string(&self) -> bool {
    matches!(self.ty, Ty::TemplateString)
  }

  pub fn is_regexp(&self) -> bool {
    matches!(self.ty, Ty::RegExp)
  }

  pub fn is_compile_time_value(&self) -> bool {
    matches!(
      self.ty,
      Ty::Undefined
        | Ty::Null
        | Ty::String
        | Ty::Number
        | Ty::Boolean
        | Ty::RegExp
        | Ty::ConstArray
        | Ty::BigInt
    )
  }

  pub fn is_nullish(&self) -> Option<bool> {
    self.nullish
  }

  pub fn is_primitive_type(&self) -> Option<bool> {
    match self.ty {
      Ty::Undefined
      | Ty::Null
      | Ty::String
      | Ty::Number
      | Ty::Boolean
      | Ty::BigInt
      | Ty::TemplateString => Some(true),
      Ty::RegExp | Ty::Array | Ty::ConstArray => Some(false),
      _ => None,
    }
  }

  pub fn as_string(&self) -> Option<std::string::String> {
    if self.is_bool() {
      Some(self.bool().to_string())
    } else if self.is_null() {
      Some("null".to_string())
    } else if self.is_string() {
      Some(self.string().to_string())
    } else {
      None
    }
  }

  pub fn as_bool(&self) -> Option<Boolean> {
    if self.truthy {
      Some(true)
    } else if self.falsy || self.nullish == Some(true) {
      Some(false)
    } else {
      self.boolean
    }
  }

  pub fn as_nullish(&self) -> Option<bool> {
    let nullish = self.is_nullish();
    if nullish == Some(true) || self.is_null() || self.is_undefined() {
      Some(true)
    } else if nullish == Some(false)
      || self.is_bool()
      || self.is_string()
      || self.is_template_string()
    {
      Some(false)
    } else {
      None
    }
  }

  pub fn compare_compile_time_value(&self, b: &BasicEvaluatedExpression) -> bool {
    if self.ty != b.ty {
      false
    } else {
      match self.ty {
        Ty::Undefined => matches!(b.ty, Ty::Undefined),
        Ty::Null => matches!(b.ty, Ty::Null),
        Ty::String => {
          b.string.as_ref().expect("should not empty")
            == self.string.as_ref().expect("should not empty")
        }
        Ty::Number => {
          b.number.as_ref().expect("should not empty")
            == self.number.as_ref().expect("should not empty")
        }
        Ty::Boolean => {
          b.boolean.as_ref().expect("should not empty")
            == self.boolean.as_ref().expect("should not empty")
        }
        Ty::RegExp => false, // FIXME: maybe we can use std::ptr::eq
        // Ty::ConstArray => {
        // },
        Ty::BigInt => {
          b.bigint.as_ref().expect("should not empty")
            == self.bigint.as_ref().expect("should not empty")
        }
        _ => unreachable!("can only compare compile-time values"),
      }
    }
  }

  pub fn could_have_side_effects(&self) -> bool {
    self.side_effects
  }

  pub fn set_side_effects(&mut self, side_effects: bool) {
    self.side_effects = side_effects
  }

  pub fn set_null(&mut self) {
    self.ty = Ty::Null;
    self.side_effects = false
  }

  pub fn set_items(&mut self, items: Vec<BasicEvaluatedExpression>) {
    self.ty = Ty::Array;
    self.side_effects = items.iter().any(|item| item.could_have_side_effects());
    self.items = Some(items);
  }

  pub fn options(&self) -> &Vec<BasicEvaluatedExpression> {
    self.options.as_ref().expect("options should not empty")
  }

  pub fn set_options(&mut self, options: Option<Vec<BasicEvaluatedExpression>>) {
    self.ty = Ty::Conditional;
    self.options = options;
    self.side_effects = true;
  }

  pub fn add_options(&mut self, options: Vec<BasicEvaluatedExpression>) {
    if let Some(old) = &mut self.options {
      old.extend(options);
    } else {
      self.ty = Ty::Conditional;
      self.options = Some(options);
      self.side_effects = true;
    }
  }

  pub fn set_bool(&mut self, boolean: Boolean) {
    self.ty = Ty::Boolean;
    self.boolean = Some(boolean);
    self.side_effects = true
  }

  pub fn set_range(&mut self, start: u32, end: u32) {
    self.range = Some(DependencyLocation::new(start, end))
  }

  pub fn set_template_string(
    &mut self,
    quasis: Vec<BasicEvaluatedExpression>,
    parts: Vec<BasicEvaluatedExpression>,
    kind: TemplateStringKind,
  ) {
    self.ty = Ty::TemplateString;
    self.quasis = Some(quasis);
    self.side_effects = parts.iter().any(|p| p.side_effects);
    self.parts = Some(parts);
    self.template_string_kind = Some(kind);
  }

  pub fn set_string(&mut self, string: String) {
    self.ty = Ty::String;
    self.string = Some(string);
    self.side_effects = false;
  }

  pub fn set_regexp(&mut self, regexp: String, flags: String) {
    self.ty = Ty::RegExp;
    self.regexp = Some((regexp, flags));
    self.side_effects = false;
  }

  pub fn string(&self) -> &String {
    self.string.as_ref().expect("make sure string exist")
  }

  pub fn regexp(&self) -> &Regexp {
    self.regexp.as_ref().expect("make sure regexp exist")
  }

  pub fn bool(&self) -> Boolean {
    self.boolean.expect("make sure bool exist")
  }

  pub fn parts(&self) -> &Vec<BasicEvaluatedExpression> {
    self
      .parts
      .as_ref()
      .expect("make sure template string exist")
  }

  pub fn range(&self) -> (u32, u32) {
    let range = self.range.expect("range should not empty");
    (range.start(), range.end())
  }
}

pub fn evaluate_to_string(value: String, start: u32, end: u32) -> BasicEvaluatedExpression {
  let mut eval = BasicEvaluatedExpression::with_range(start, end);
  eval.set_string(value);
  eval
}

bitflags! {
  struct RegExpFlag: u8 {
    const FLAG_Y = 1 << 0;
    const FLAG_M = 1 << 1;
    const FLAG_I = 1 << 2;
    const FLAG_G = 1 << 3;
  }
}

pub fn is_valid_reg_exp_flags(flags: &str) -> bool {
  if flags.is_empty() {
    true
  } else if flags.len() > 4 {
    false
  } else {
    let mut remaining = RegExpFlag::empty();
    for c in flags.as_bytes() {
      match *c {
        b'g' => {
          if remaining.contains(RegExpFlag::FLAG_G) {
            return false;
          }
          remaining.insert(RegExpFlag::FLAG_G);
        }
        b'i' => {
          if remaining.contains(RegExpFlag::FLAG_I) {
            return false;
          }
          remaining.insert(RegExpFlag::FLAG_I);
        }
        b'm' => {
          if remaining.contains(RegExpFlag::FLAG_M) {
            return false;
          }
          remaining.insert(RegExpFlag::FLAG_M);
        }
        b'y' => {
          if remaining.contains(RegExpFlag::FLAG_Y) {
            return false;
          }
          remaining.insert(RegExpFlag::FLAG_Y);
        }
        _ => return false,
      }
    }
    true
  }
}
