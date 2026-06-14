#[macro_export]
macro_rules! dex_id {
    ($ident:ident, $repr:ty) => {
        #[doc = r"ID guaranteed to be valid as the dex is immutable after deserialization"]
        #[derive(
            Debug,
            Clone,
            Copy,
            serde::Deserialize,
            serde::Serialize,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
        )]
        #[repr(transparent)]
        pub struct $ident(pub(crate) $repr);

        impl $crate::infinite_fusion::DexId for $ident {
            fn from_usize(v: usize) -> Self {
                Self(v as $repr)
            }

            fn to_usize(self) -> usize {
                self.0 as usize
            }

            fn from_u32(v: u32) -> Self {
                Self(v as $repr)
            }

            fn to_u32(self) -> u32 {
                self.0 as u32
            }
        }
    };
}
