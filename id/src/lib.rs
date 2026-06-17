/// 이 크레이트를 사용하기 위해서는 반드시 아래와 같이 Cargo.toml에 serde를 추가해야 한다.
/// serde = { version = "1.0.0", features = ["derive"] }
#[macro_export]
macro_rules! ID {
    ($ty:ident, $inner:ty, $default:expr) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $ty(pub $inner);

        impl $ty {
            pub fn id(&self) -> $inner {
                self.0
            }
        }

        impl Default for $ty {
            fn default() -> Self {
                Self($default)
            }
        }

        impl From<$ty> for $inner {
            fn from(value: $ty) -> Self {
                value.0
            }
        }

        impl From<$inner> for $ty {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl std::str::FromStr for $ty {
            type Err = <$inner as std::str::FromStr>::Err;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse::<$inner>().map(Self)
            }
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}
