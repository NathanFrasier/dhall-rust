use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dhall::{Ctxt, Parsed};

use crate::options::{HasAnnot, ManualAnnot, NoAnnot, StaticAnnot, TypeAnnot};
use crate::SimpleType;
use crate::{Error, ErrorKind, FromDhall, Result, Value};

#[derive(Debug, Clone)]
enum Source<'a> {
    Str(&'a str),
    File(PathBuf),
    BinaryFile(PathBuf),
    // Url(&'a str),
}

/// Controls how a Dhall value is read.
///
/// This builder exposes the ability to configure how a value is deserialized and what operations
/// are permitted during evaluation.
///
/// Generally speaking, when using [`Deserializer`], you'll create it with [`from_str()`] or
/// [`from_file()`], then chain calls to methods to set each option, then call [`parse()`]. This
/// will give you a [`Result<T>`] where `T` is a deserializable type of your choice.
///
/// [`parse()`]: Deserializer::parse()
/// [`Result<T>`]: Result
///
/// # Examples
///
/// Reading from a file:
///
/// ```no_run
/// # fn main() -> serde_dhall::Result<()> {
/// use serde_dhall::from_file;
///
/// let data = from_file("foo.dhall").parse::<u64>()?;
/// # Ok(())
/// # }
/// ```
///
/// Reading from a file and checking the value against a provided type:
///
/// ```no_run
/// # fn main() -> serde_dhall::Result<()> {
/// use std::collections::HashMap;
/// use serde_dhall::{from_file, from_str};
///
/// let ty = from_str("{ x: Natural, y: Natural }").parse()?;
/// let data = from_file("foo.dhall")
///             .type_annotation(&ty)
///             .parse::<HashMap<String, u64>>()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Deserializer<'a, A> {
    source: Source<'a>,
    annot: A,
    allow_imports: bool,
    builtins: HashMap<dhall::syntax::Label, dhall::syntax::Expr>,
    // allow_remote_imports: bool,
    // use_cache: bool,
}

impl<'a> Deserializer<'a, NoAnnot> {
    fn default_with_source(source: Source<'a>) -> Self {
        Deserializer {
            source,
            annot: NoAnnot,
            allow_imports: true,
            builtins: HashMap::new(),
            // allow_remote_imports: true,
            // use_cache: true,
        }
    }
    fn from_str(s: &'a str) -> Self {
        Self::default_with_source(Source::Str(s))
    }
    fn from_file<P: AsRef<Path>>(path: P) -> Self {
        Self::default_with_source(Source::File(path.as_ref().to_owned()))
    }
    fn from_binary_file<P: AsRef<Path>>(path: P) -> Self {
        Self::default_with_source(Source::BinaryFile(path.as_ref().to_owned()))
    }
    // fn from_url(url: &'a str) -> Self {
    //     Self::default_with_source(Source::Url(url))
    // }

    /// Ensures that the parsed value matches the provided type.
    ///
    /// In many cases the Dhall type that corresponds to a Rust type can be inferred automatically.
    /// See the [`StaticType`] trait and the [`static_type_annotation()`] method for that.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> serde_dhall::Result<()> {
    /// use std::collections::HashMap;
    /// use serde::Deserialize;
    /// use serde_dhall::{from_str, SimpleType};
    ///
    /// // Parse a Dhall type
    /// let type_str = "{ x: Natural, y: Natural }";
    /// let ty = from_str(type_str).parse::<SimpleType>()?;
    ///
    /// // Parse some Dhall data.
    /// let data = "{ x = 1, y = 1 + 1 }";
    /// let point = from_str(data)
    ///     .type_annotation(&ty)
    ///     .parse::<HashMap<String, u64>>()?;
    /// assert_eq!(point.get("y"), Some(&2));
    ///
    /// // Invalid data fails the type validation; deserialization would have succeeded otherwise.
    /// let invalid_data = "{ x = 1, z = 3 }";
    /// assert!(
    ///     from_str(invalid_data)
    ///         .type_annotation(&ty)
    ///         .parse::<HashMap<String, u64>>()
    ///         .is_err()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`StaticType`]: crate::StaticType
    /// [`static_type_annotation()`]: Deserializer::static_type_annotation()
    pub fn type_annotation<'ty>(
        self,
        ty: &'ty SimpleType,
    ) -> Deserializer<'a, ManualAnnot<'ty>> {
        Deserializer {
            annot: ManualAnnot(ty),
            source: self.source,
            allow_imports: self.allow_imports,
            builtins: self.builtins,
        }
    }

    /// Ensures that the parsed value matches the type of `T`.
    ///
    /// `T` must implement the [`StaticType`] trait. If it doesn't, you can use
    /// [`type_annotation()`] to provide a type manually.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> serde_dhall::Result<()> {
    /// use serde::Deserialize;
    /// use serde_dhall::StaticType;
    ///
    /// #[derive(Deserialize, StaticType)]
    /// struct Point {
    ///     x: u64,
    ///     y: Option<u64>,
    /// }
    ///
    /// // Some Dhall data
    /// let data = "{ x = 1, y = Some (1 + 1) }";
    ///
    /// // Convert the Dhall string to a Point.
    /// let point = serde_dhall::from_str(data)
    ///     .static_type_annotation()
    ///     .parse::<Point>()?;
    /// assert_eq!(point.x, 1);
    /// assert_eq!(point.y, Some(2));
    ///
    /// // Invalid data fails the type validation; deserialization would have succeeded otherwise.
    /// let invalid_data = "{ x = 1 }";
    /// assert!(
    ///     serde_dhall::from_str(invalid_data)
    ///         .static_type_annotation()
    ///         .parse::<Point>()
    ///         .is_err()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`StaticType`]: crate::StaticType
    /// [`type_annotation()`]: Deserializer::type_annotation()
    pub fn static_type_annotation(self) -> Deserializer<'a, StaticAnnot> {
        Deserializer {
            annot: StaticAnnot,
            source: self.source,
            allow_imports: self.allow_imports,
            builtins: self.builtins,
        }
    }
}

impl<'a, A> Deserializer<'a, A> {
    /// Sets whether to enable imports.
    ///
    /// By default, imports are enabled.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> serde_dhall::Result<()> {
    /// use serde::Deserialize;
    /// use serde_dhall::SimpleType;
    ///
    /// let data = "12 + ./other_file.dhall : Natural";
    /// assert!(
    ///     serde_dhall::from_str(data)
    ///         .imports(false)
    ///         .parse::<u64>()
    ///         .is_err()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn imports(self, imports: bool) -> Self {
        Deserializer {
            allow_imports: imports,
            ..self
        }
    }

    // /// TODO
    // pub fn remote_imports(&mut self, imports: bool) -> &mut Self {
    //     self.allow_remote_imports = imports;
    //     if imports {
    //         self.allow_imports = true;
    //     }
    //     self
    // }

    /// Makes a set of types available to the parsed dhall code. This is similar to how builtins
    /// like `Natural` work: they are provided by dhall and accessible in any file.
    ///
    /// This is especially useful when exposing rust types exposing the rust types to dhall, since
    /// this avoids having to define them in both languages and keep both definitions in sync.
    ///
    /// Warning: the new builtins will only be accessible to the current file. If this file has
    /// imports, the imported values will not have access to the builtins.
    ///
    /// See also [`with_builtin_type()`].
    /// [`with_builtin_type()`]: Deserializer::with_builtin_type()
    ///
    /// # Example
    /// ```
    /// use serde::Deserialize;
    /// use serde_dhall::StaticType;
    /// use std::collections::HashMap;
    ///
    /// #[derive(Deserialize, StaticType, Debug, PartialEq)]
    /// enum Newtype {
    ///   Foo,
    ///   Bar(u64)
    /// }
    ///
    /// let mut builtins = HashMap::new();
    /// builtins.insert("Newtype".to_string(), Newtype::static_type());
    ///
    /// let data = "Newtype.Bar 0";
    ///
    /// let deserialized = serde_dhall::from_str(data)
    ///   .with_builtin_types(builtins)
    ///   .parse::<Newtype>()
    ///   .unwrap();
    ///
    /// assert_eq!(deserialized, Newtype::Bar(0));
    /// ```
    pub fn with_builtin_types(
        mut self,
        tys: impl IntoIterator<Item = (String, SimpleType)>,
    ) -> Self {
        self.builtins.extend(
            tys.into_iter().map(|(s, ty)| {
                (dhall::syntax::Label::from_str(&s), ty.to_expr())
            }),
        );
        self
    }

    /// Makes a type available to the parsed dhall code. This is similar to how builtins like
    /// `Natural` work: they are provided by dhall and accessible in any file.
    ///
    /// This is especially useful when exposing rust types exposing the rust types to dhall, since
    /// this avoids having to define them in both languages and keep both definitions in sync.
    ///
    /// Warning: the new builtins will only be accessible to the current file. If this file has
    /// imports, the imported values will not have access to the builtins.
    ///
    /// See also [`with_builtin_types()`].
    /// [`with_builtin_types()`]: Deserializer::with_builtin_types()
    ///
    /// # Example
    /// ```
    /// use serde::Deserialize;
    /// use serde_dhall::StaticType;
    /// use std::collections::HashMap;
    ///
    /// #[derive(Deserialize, StaticType, Debug, PartialEq)]
    /// enum Newtype {
    ///   Foo,
    ///   Bar(u64)
    /// }
    ///
    /// let data = "Newtype.Bar 0";
    ///
    /// let deserialized = serde_dhall::from_str(data)
    ///   .with_builtin_type("Newtype".to_string(), Newtype::static_type())
    ///   .parse::<Newtype>()
    ///   .unwrap();
    ///
    /// assert_eq!(deserialized, Newtype::Bar(0));
    /// ```
    pub fn with_builtin_type(mut self, name: String, ty: SimpleType) -> Self {
        self.builtins
            .insert(dhall::syntax::Label::from_str(&name), ty.to_expr());
        self
    }

    fn _parse<T>(&self) -> dhall::error::Result<Result<Value>>
    where
        A: TypeAnnot,
        T: HasAnnot<A>,
    {
        Ctxt::with_new(|cx| {
            let parsed = match &self.source {
                Source::Str(s) => Parsed::parse_str(s)?,
                Source::File(p) => Parsed::parse_file(p.as_ref())?,
                Source::BinaryFile(p) => Parsed::parse_binary_file(p.as_ref())?,
            };

            let parsed_with_builtins =
                self.builtins.iter().fold(parsed, |acc, (name, subst)| {
                    acc.add_let_binding(name.clone(), subst.clone())
                });

            let resolved = if self.allow_imports {
                parsed_with_builtins.resolve(cx)?
            } else {
                parsed_with_builtins.skip_resolve(cx)?
            };
            let typed = match &T::get_annot(self.annot) {
                None => resolved.typecheck(cx)?,
                Some(ty) => resolved.typecheck_with(cx, &ty.to_hir())?,
            };
            Ok(Value::from_nir_and_ty(
                cx,
                typed.normalize(cx).as_nir(),
                typed.ty().as_nir(),
            ))
        })
    }

    /// Parses the chosen dhall value with the options provided.
    ///
    /// If you enabled static annotations, `T` is required to implement [`StaticType`].
    ///
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> serde_dhall::Result<()> {
    /// let data = serde_dhall::from_str("6 * 7").parse::<u64>()?;
    /// assert_eq!(data, 42);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`StaticType`]: crate::StaticType
    pub fn parse<T>(&self) -> Result<T>
    where
        A: TypeAnnot,
        T: FromDhall + HasAnnot<A>,
    {
        let val = self
            ._parse::<T>()
            .map_err(ErrorKind::Dhall)
            .map_err(Error)??;
        T::from_dhall(&val)
    }
}

/// Deserialize a value from a string of Dhall text.
///
/// This returns a [`Deserializer`] object. Call the [`parse()`] method to get the deserialized
/// value, or use other [`Deserializer`] methods to control the deserialization process.
///
/// Imports will be resolved relative to the current directory.
///
/// # Example
///
/// ```rust
/// # fn main() -> serde_dhall::Result<()> {
/// use serde::Deserialize;
///
/// // We use serde's derive feature
/// #[derive(Deserialize)]
/// struct Point {
///     x: u64,
///     y: u64,
/// }
///
/// // Some Dhall data
/// let data = "{ x = 1, y = 1 + 1 } : { x: Natural, y: Natural }";
///
/// // Parse the Dhall string as a Point.
/// let point: Point = serde_dhall::from_str(data).parse()?;
///
/// assert_eq!(point.x, 1);
/// assert_eq!(point.y, 2);
/// # Ok(())
/// # }
/// ```
///
/// [`parse()`]: Deserializer::parse()
pub fn from_str(s: &str) -> Deserializer<'_, NoAnnot> {
    Deserializer::from_str(s)
}

/// Deserialize a value from a Dhall file.
///
/// This returns a [`Deserializer`] object. Call the [`parse()`] method to get the deserialized
/// value, or use other [`Deserializer`] methods to control the deserialization process.
///
/// Imports will be resolved relative to the provided file's path.
///
/// # Example
///
/// ```no_run
/// # fn main() -> serde_dhall::Result<()> {
/// use serde::Deserialize;
///
/// // We use serde's derive feature
/// #[derive(Deserialize)]
/// struct Point {
///     x: u64,
///     y: u64,
/// }
///
/// // Parse the Dhall file as a Point.
/// let point: Point = serde_dhall::from_file("foo.dhall").parse()?;
/// # Ok(())
/// # }
/// ```
///
/// [`parse()`]: Deserializer::parse()
pub fn from_file<'a, P: AsRef<Path>>(path: P) -> Deserializer<'a, NoAnnot> {
    Deserializer::from_file(path)
}

/// Deserialize a value from a CBOR-encoded Dhall binary file. The binary format is specified by
/// the Dhall standard specification and is mostly used for caching expressions. Using the format
/// is not recommended because errors won't have a file to refer to and thus will be hard to fix.
///
/// This returns a [`Deserializer`] object. Call the [`parse()`] method to get the deserialized
/// value, or use other [`Deserializer`] methods to control the deserialization process.
///
/// Imports will be resolved relative to the provided file's path.
///
/// # Example
///
/// ```no_run
/// # fn main() -> serde_dhall::Result<()> {
/// use serde::Deserialize;
///
/// // We use serde's derive feature
/// #[derive(Deserialize)]
/// struct Point {
///     x: u64,
///     y: u64,
/// }
///
/// // Parse the Dhall file as a Point.
/// let point: Point = serde_dhall::from_binary_file("foo.dhallb").parse()?;
/// # Ok(())
/// # }
/// ```
///
/// [`parse()`]: Deserializer::parse()
pub fn from_binary_file<'a, P: AsRef<Path>>(
    path: P,
) -> Deserializer<'a, NoAnnot> {
    Deserializer::from_binary_file(path)
}

// pub fn from_url(url: &str) -> Deserializer<'_, NoAnnot> {
//     Deserializer::from_url(url)
// }
