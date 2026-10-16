//! Values: everything a template can see, and how hosts build them.

use std::cell::{Cell, Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use crate::calls::Call;
use crate::convert;
use crate::error::Error;
use crate::runtime::{BlockReference, Macro, Recursion};

/// The one signature of every filter, test, global function and method written in Rust.
/// Report errors by returning `Err(call.error("..."))`.
pub type Native = Rc<dyn Fn(&mut Call<'_, '_>) -> Result<Value, Error>>;

/// What a missing variable, attribute or item evaluates to. The fields only feed the
/// error message produced when the value is used in a way Jinja forbids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Undefined {
    /// The missing variable or attribute.
    pub name: Rc<str>,
    /// `"dict object"` when an attribute or item was missing, else empty.
    pub owner: Rc<str>,
    /// A complete message that replaces the generated one, or empty.
    pub hint: Rc<str>,
}

/// Every value a template can see. The default value is Undefined.
///
/// Lists, dicts and the other containers are shared (`Rc`): cloning a value shares it,
/// as Python shares objects.
#[derive(Clone)]
pub enum Value {
    /// A missing value, with the details its error message names when there are any.
    Undefined(Option<Rc<Undefined>>),
    /// Python's `None`.
    None,
    /// `True` or `False`.
    Bool(bool),
    /// A 64-bit integer; overflow is an error, not a wrap.
    Int(i64),
    /// A float.
    Float(f64),
    /// A string.
    Str(Rc<str>),
    /// A string that is already safe HTML: autoescaping leaves it alone.
    Markup(Rc<str>),
    /// A list or a tuple.
    List(Rc<List>),
    /// A dict (ordered), a namespace or an imported module.
    Dict(Rc<Dict>),
    /// A function: native, a bound method, a macro or a block reference.
    Callable(Rc<Callable>),
    /// The `loop` variable of a for loop.
    Loop(Rc<Loop>),
    /// The object `cycler()` makes.
    Cycler(Rc<Cycler>),
}

/// A list, or a tuple when `is_tuple`. A named tuple (the groups `groupby` makes) can
/// also be read through its field names.
pub struct List {
    items: RefCell<Vec<Value>>,
    tuple: bool,
    pub(crate) fields: &'static [&'static str],
}

/// What a [`Dict`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictKind {
    /// A plain dict.
    #[default]
    Dict,
    /// `namespace()`: the only object whose attributes templates may assign.
    Namespace,
    /// The result of `{% import %}`: exported macros and variables.
    Module,
}

/// An insertion-ordered dictionary. String keys are indexed; other keys are compared
/// with Python equality (`1 == 1.0 == True`).
pub struct Dict {
    map: RefCell<DictMap>,
    kind: DictKind,
    name: Rc<str>,
    reads: RefCell<Option<HashSet<String>>>,
}

#[derive(Default)]
struct DictMap {
    keys: Vec<Value>,
    values: Vec<Value>,
    index: HashMap<Rc<str>, usize>,
}

/// A function value.
pub struct Callable {
    pub(crate) name: Rc<str>,
    pub(crate) kind: CallKind,
    /// The render that made it, or 0 for one any render may call.
    pub(crate) render: u64,
}

pub(crate) enum CallKind {
    Native(Native),
    Method(Value),
    Macro(Rc<Macro>),
    Block(Rc<BlockReference>),
    /// A function of the render's `Locals`, found again by the callable's name.
    Local,
}

/// The special `loop` variable of a for loop.
pub struct Loop {
    pub(crate) items: Vec<Value>,
    pub(crate) index0: Cell<usize>,
    pub(crate) depth0: usize,
    pub(crate) last_seen: RefCell<Option<Value>>,
    pub(crate) recurse: Option<Rc<Recursion>>,
}

/// The object made by the `cycler()` global.
pub struct Cycler {
    pub(crate) items: Vec<Value>,
    pub(crate) pos: Cell<usize>,
}

impl List {
    /// Makes a shared list (or tuple) holding `items`.
    pub fn new(items: Vec<Value>, tuple: bool) -> Rc<List> {
        Rc::new(List { items: RefCell::new(items), tuple, fields: &[] })
    }

    pub(crate) fn named(items: Vec<Value>, fields: &'static [&'static str]) -> Rc<List> {
        Rc::new(List { items: RefCell::new(items), tuple: true, fields })
    }

    /// Borrows the items. Panics if a template is changing the list at that moment,
    /// which cannot happen outside a native procedure that changes it itself.
    pub fn items(&self) -> Ref<'_, Vec<Value>> {
        self.items.borrow()
    }

    /// Copies the item handles (the items stay shared).
    pub fn snapshot(&self) -> Vec<Value> {
        self.items.borrow().clone()
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether the list has no items.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether this is a tuple.
    pub fn is_tuple(&self) -> bool {
        self.tuple
    }

    /// Item `i`, if there is one.
    pub fn item(&self, i: usize) -> Option<Value> {
        self.items.borrow().get(i).cloned()
    }

    /// Appends items to the end.
    pub fn extend(&self, items: Vec<Value>) {
        self.items.borrow_mut().extend(items);
    }

    /// Appends one item.
    pub fn push(&self, item: Value) {
        self.items.borrow_mut().push(item);
    }

    /// Inserts item before position at (at most the length).
    pub(crate) fn insert(&self, at: usize, item: Value) {
        self.items.borrow_mut().insert(at, item);
    }

    /// Removes and returns item i, which exists.
    pub(crate) fn remove(&self, i: usize) -> Value {
        self.items.borrow_mut().remove(i)
    }

    /// Reverses the items in place.
    pub(crate) fn reverse(&self) {
        self.items.borrow_mut().reverse();
    }

    /// Replaces all items; the old ones are dropped after the list is released, since
    /// they may hold the list itself.
    pub(crate) fn replace(&self, items: Vec<Value>) {
        let old = std::mem::replace(&mut *self.items.borrow_mut(), items);
        drop_values(old);
    }
}

impl Drop for List {
    fn drop(&mut self) {
        drop_values(std::mem::take(self.items.get_mut()));
    }
}

/// drop_values drops nested lists and dicts without recursing, so that a value nested
/// thousands of levels deep cannot overflow the stack when it goes.
fn drop_values(mut stack: Vec<Value>) {
    while let Some(v) = stack.pop() {
        match v {
            Value::List(rc) => {
                if let Ok(mut l) = Rc::try_unwrap(rc) {
                    stack.append(l.items.get_mut());
                }
            }
            Value::Dict(rc) => {
                if let Ok(mut d) = Rc::try_unwrap(rc) {
                    let map = d.map.get_mut();
                    stack.append(&mut map.keys);
                    stack.append(&mut map.values);
                }
            }
            _ => {}
        }
    }
}

impl Dict {
    /// Makes an empty shared dict of `kind`.
    pub fn new(kind: DictKind) -> Rc<Dict> {
        Rc::new(Dict {
            map: RefCell::default(),
            kind,
            name: Rc::from(""),
            reads: RefCell::default(),
        })
    }

    pub(crate) fn module(name: Rc<str>) -> Rc<Dict> {
        Rc::new(Dict {
            map: RefCell::default(),
            kind: DictKind::Module,
            name,
            reads: RefCell::default(),
        })
    }

    /// Starts recording string-key reads, including missing keys. Replaces any prior trace.
    pub fn begin_key_trace(&self) {
        *self.reads.borrow_mut() = Some(HashSet::new());
    }

    /// Stops recording and returns distinct keys in sorted order; empty if tracing was off.
    pub fn end_key_trace(&self) -> Vec<String> {
        let mut keys: Vec<_> =
            self.reads.borrow_mut().take().unwrap_or_default().into_iter().collect();
        keys.sort();
        keys
    }

    fn note_key(&self, key: &str) {
        if let Some(keys) = self.reads.borrow_mut().as_mut() {
            keys.insert(key.into());
        }
    }

    fn note_all(&self) {
        if let Some(keys) = self.reads.borrow_mut().as_mut() {
            keys.extend(
                self.map.borrow().keys.iter().filter_map(|k| k.as_str().map(str::to_owned)),
            );
        }
    }

    /// What the dict is.
    pub fn kind(&self) -> DictKind {
        self.kind
    }

    /// The template name of a module; empty otherwise.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.map.borrow().keys.len()
    }

    /// Whether the dict has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn find(&self, key: &Value) -> Option<usize> {
        let map = self.map.borrow();
        if let Some(s) = key.as_str() {
            return map.index.get(s).copied();
        }
        map.keys.iter().position(|k| k.as_str().is_none() && convert::values_equal(k, key))
    }

    /// The value stored under `key`.
    pub fn get(&self, key: &Value) -> Option<Value> {
        if let Some(key) = key.as_str() {
            self.note_key(key);
        }
        let i = self.find(key)?;
        Some(self.map.borrow().values[i].clone())
    }

    /// The value stored under the string `key`.
    pub fn get_str(&self, key: &str) -> Option<Value> {
        self.note_key(key);
        let map = self.map.borrow();
        map.index.get(key).map(|&i| map.values[i].clone())
    }

    /// Adds or replaces an entry.
    pub fn set(&self, key: Value, value: Value) {
        if let Some(i) = self.find(&key) {
            self.map.borrow_mut().values[i] = value;
            return;
        }
        let mut map = self.map.borrow_mut();
        let n = map.keys.len();
        if let Some(s) = key.as_str() {
            map.index.insert(Rc::from(s), n);
        }
        map.keys.push(key);
        map.values.push(value);
    }

    /// Removes the entry under `key` and returns its value.
    pub fn remove(&self, key: &Value) -> Option<Value> {
        let i = self.find(key)?;
        let mut map = self.map.borrow_mut();
        map.keys.remove(i);
        let v = map.values.remove(i);
        let DictMap { keys, index, .. } = &mut *map;
        index.clear();
        for (j, k) in keys.iter().enumerate() {
            if let Some(s) = k.as_str() {
                index.insert(Rc::from(s), j);
            }
        }
        Some(v)
    }

    /// Removes every entry.
    pub fn clear(&self) {
        let mut map = self.map.borrow_mut();
        let old = std::mem::take(&mut *map);
        drop(map);
        drop_values(old.keys.into_iter().chain(old.values).collect());
    }

    /// The keys, in insertion order (shared handles).
    pub fn keys(&self) -> Vec<Value> {
        self.note_all();
        self.map.borrow().keys.clone()
    }

    /// The values, in insertion order (shared handles).
    pub fn values(&self) -> Vec<Value> {
        self.note_all();
        self.map.borrow().values.clone()
    }

    /// The entries, in insertion order (shared handles).
    pub fn entries(&self) -> Vec<(Value, Value)> {
        self.note_all();
        let map = self.map.borrow();
        map.keys.iter().cloned().zip(map.values.iter().cloned()).collect()
    }

    /// Calls `f` with each entry without copying them.
    pub(crate) fn each<R>(&self, f: impl FnOnce(&[Value], &[Value]) -> R) -> R {
        self.note_all();
        let map = self.map.borrow();
        f(&map.keys, &map.values)
    }

    pub(crate) fn copy(&self, kind: DictKind) -> Rc<Dict> {
        let out = Dict::new(kind);
        for (k, v) in self.entries() {
            out.set(k, v);
        }
        out
    }
}

impl Drop for Dict {
    fn drop(&mut self) {
        let map = std::mem::take(self.map.get_mut());
        drop_values(map.keys.into_iter().chain(map.values).collect());
    }
}

impl Callable {
    /// The name the function was registered or defined with.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this is a macro (or a call block's `caller`).
    pub fn is_macro(&self) -> bool {
        matches!(self.kind, CallKind::Macro(_))
    }

    pub(crate) fn value(name: Rc<str>, kind: CallKind, render: u64) -> Value {
        Value::Callable(Rc::new(Callable { name, kind, render }))
    }
}

impl Loop {
    /// The 0-based index of the current iteration.
    pub fn index0(&self) -> usize {
        self.index0.get()
    }

    /// The number of items the loop runs over.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the loop runs over no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------------------
// Building and reading values

impl Value {
    /// Makes a string value.
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    /// Makes a string value that is safe HTML, which autoescaping leaves alone.
    pub fn markup(s: &str) -> Value {
        Value::Markup(Rc::from(s))
    }

    /// Makes an undefined value; `name` is what error messages call it.
    pub fn undefined(name: &str) -> Value {
        Value::Undefined(Some(Rc::new(Undefined { name: Rc::from(name), ..Undefined::default() })))
    }

    pub(crate) fn undefined_with(name: &str, owner: &str, hint: &str) -> Value {
        Value::Undefined(Some(Rc::new(Undefined {
            name: Rc::from(name),
            owner: Rc::from(owner),
            hint: Rc::from(hint),
        })))
    }

    pub(crate) fn undefined_hint(hint: &str) -> Value {
        Value::undefined_with("", "", hint)
    }

    /// Makes a list holding `items`.
    pub fn list(items: impl IntoIterator<Item = Value>) -> Value {
        Value::List(List::new(items.into_iter().collect(), false))
    }

    /// Makes a tuple holding `items`.
    pub fn tuple(items: impl IntoIterator<Item = Value>) -> Value {
        Value::List(List::new(items.into_iter().collect(), true))
    }

    /// Makes an empty dict; fill it with [`Value::set`].
    pub fn dict() -> Value {
        Value::Dict(Dict::new(DictKind::Dict))
    }

    /// Wraps a native procedure as a callable value for globals or context data.
    pub fn function(
        name: &str,
        f: impl Fn(&mut Call<'_, '_>) -> Result<Value, Error> + 'static,
    ) -> Value {
        Callable::value(Rc::from(name), CallKind::Native(Rc::new(f)), 0)
    }

    /// Stores `value` under the string `key` of a dict value; nothing happens when this
    /// is not a dict.
    pub fn set(&self, key: &str, value: Value) {
        if let Value::Dict(d) = self {
            d.set(Value::str(key), value);
        }
    }

    /// Stores `value` under any key of a dict value.
    pub fn set_key(&self, key: Value, value: Value) {
        if let Value::Dict(d) = self {
            d.set(key, value);
        }
    }

    /// Reads a string key from a dict value; missing keys give Undefined.
    pub fn get(&self, key: &str) -> Value {
        if let Value::Dict(d) = self
            && let Some(v) = d.get_str(key)
        {
            return v;
        }
        Value::undefined(key)
    }

    /// The string keys of a dict value, in insertion order.
    pub fn keys(&self) -> Vec<String> {
        let Value::Dict(d) = self else { return Vec::new() };
        d.keys().iter().filter_map(|k| k.as_str().map(String::from)).collect()
    }

    /// Appends `item` to a list value; nothing happens when this is not a list.
    pub fn push(&self, item: Value) {
        if let Value::List(l) = self {
            l.push(item);
        }
    }

    /// The number of items of a list or dict, or of characters of a string; 0 otherwise.
    pub fn len(&self) -> usize {
        match self {
            Value::Str(s) | Value::Markup(s) => s.chars().count(),
            Value::List(l) => l.len(),
            Value::Dict(d) => d.len(),
            _ => 0,
        }
    }

    /// Whether [`Value::len`] is 0.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The text of a string or Markup value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Markup(s) => Some(s),
            _ => None,
        }
    }

    /// Whether the value is undefined.
    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined(_))
    }

    /// Whether the value is None.
    pub fn is_none(&self) -> bool {
        matches!(self, Value::None)
    }

    /// Whether the value is safe HTML.
    pub fn is_markup(&self) -> bool {
        matches!(self, Value::Markup(_))
    }

    /// Whether the value is a string or Markup.
    pub fn is_string(&self) -> bool {
        matches!(self, Value::Str(_) | Value::Markup(_))
    }

    /// Python truthiness (Undefined is false).
    pub fn is_truthy(&self) -> bool {
        convert::truthy(self)
    }

    /// The Python type name used in Jinja's error messages (`"str"`, `"dict"`, ...).
    pub fn type_name(&self) -> &'static str {
        convert::type_name(self)
    }

    /// The value as Python's `repr()` writes it.
    pub fn repr(&self) -> String {
        convert::repr(self)
    }

    /// The details of an undefined value (empty ones for a plain Undefined).
    pub(crate) fn undefined_info(&self) -> Option<Undefined> {
        match self {
            Value::Undefined(None) => Some(Undefined::default()),
            Value::Undefined(Some(u)) => Some((**u).clone()),
            _ => None,
        }
    }
}

impl Default for Value {
    /// Undefined, with no details.
    fn default() -> Value {
        Value::Undefined(None)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::str(s)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(Rc::from(s))
    }
}

impl From<Rc<str>> for Value {
    fn from(s: Rc<str>) -> Value {
        Value::Str(s)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Value {
        Value::Int(i)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Value {
        Value::Int(i64::from(i))
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Value {
        Value::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

impl fmt::Display for Value {
    /// Writes the value as Python's `str()` does (Undefined writes nothing).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&convert::to_str(self))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&convert::repr(self))
    }
}

impl PartialEq for Value {
    /// Python's `==`: lists and dicts compare by content.
    fn eq(&self, other: &Value) -> bool {
        convert::values_equal(self, other)
    }
}

impl fmt::Debug for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        convert::write_list_repr(&mut out, self, 0);
        f.write_str(&out)
    }
}

impl fmt::Debug for Dict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        convert::write_dict_repr(&mut out, self, 0);
        f.write_str(&out)
    }
}

impl fmt::Debug for Callable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function {}>", self.name)
    }
}

impl fmt::Debug for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LoopContext {}/{}>", self.index0.get() + 1, self.items.len())
    }
}

impl fmt::Debug for Cycler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<Cycler object>")
    }
}
