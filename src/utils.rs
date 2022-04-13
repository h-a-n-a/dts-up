use smol_str::SmolStr;
use std::fmt::Display;

#[inline]
pub fn resolve_id(id: &str) -> SmolStr {
  if id.ends_with(".d.ts") {
    SmolStr::from(id)
  } else {
    let mut str = id.to_owned();
    str.push_str(".d.ts");
    SmolStr::new(str)
  }
}

// #[macro_export]
// macro_rules! resolve_dts {
//   ( $( $x:expr ),* ) => {
//     {
//       let mut p: String = Default::default();
//
//       $({
//         p.push_str($x);
//       })*
//
//       // to be implemented
//     }
//   };
// }
//
// pub use resolve_dts;
