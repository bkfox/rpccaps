#![feature(associated_type_defaults)]
#![feature(async_closure)]
#![warn(unused_extern_crates)]

pub mod data;
pub mod rpc;


pub mod tests {
    #[macro_export]
    macro_rules! expect {
        ($test: expr, $result: pat) => {
            let r = $test;
            match r {
                $result => (),
                _ => panic!("expected {:?}, got {:?}", stringify!($result), r),
            }
        }
    }
}

