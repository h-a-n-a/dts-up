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

macro_rules! resolve_dts {
  ( $( $x:expr ),* ) => {
    {
      let str = vec![];

      $({
        str.push($x);
      })*

      println!("{:?}", str);

      // $relative_path::RelativePath::new(str.)
    }
  };
}

pub use resolve_dts;
