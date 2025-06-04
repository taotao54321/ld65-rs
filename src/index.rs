//! 各種インデックスの定義。
//!
//! 配列ベースで管理するもの (メモリ領域、セグメントなど) が多々あるので、
//! インデックスの取り違え事故を防ぐためにそれぞれ専用のインデックスを設ける。

macro_rules! define_index {
    ($ty:ident) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $ty(usize);

        impl $ty {
            pub fn new(inner: usize) -> Self {
                Self(inner)
            }

            pub fn get(self) -> usize {
                self.0
            }
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

// 出力ファイルのインデックス。
define_index!(OutFileIdx);

// メモリ領域のインデックス。
define_index!(MemIdx);

// セグメントのインデックス。
define_index!(SegIdx);

// 全オブジェクトファイルを通じたセクションのインデックス。
define_index!(SectIdx);

// オブジェクトファイルのインデックス。
define_index!(ObjIdx);

// オブジェクトファイル内ローカルなセクションのインデックス。
define_index!(ObjSectIdx);

// オブジェクトファイル内ローカルなインポートインデックス。
define_index!(ObjImportIdx);

// オブジェクトファイル内ローカルな文字列インデックス。
define_index!(ObjStrIdx);
