use std::fmt::Display;

use smol_str::SmolStr;

pub use swc_atoms::js_word;

#[inline]
pub fn resolve_id(id: &str) -> SmolStr {
  if id.ends_with(".d.ts") {
    SmolStr::from(id)
  } else if id.ends_with(".d") {
    let mut str = id.to_owned();
    str.push_str(".ts");
    SmolStr::new(str)
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
