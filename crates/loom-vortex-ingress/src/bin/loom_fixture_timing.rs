//! Illustrative wall-clock timing for Loom interpreter vs Vortex oracle decode.
//!
//! This is not a benchmark. It is a reviewer-facing sanity aid for Phase 7.

use std::hint::black_box;
use std::time::{Duration, Instant};

use arrow_schema::DataType;
use loom_ffi::l1_model::{decode_layout_to_array_data, LayoutDescription};
use loom_ffi::l2_kernel_registry::L2KernelRegistry;
use vortex_array::arrays::{PrimitiveArray, VarBinArray};
use vortex_array::dtype::{DType, Nullability};
use vortex_array::{IntoArray, VortexSessionExecute, LEGACY_SESSION};
use vortex_fastlanes::BitPackedData;

use loom_vortex_ingress::{oracle, vortex_reader};

fn main() {
    println!("Loom fixture timing (illustrative wall-clock, not a benchmark)");
    println!("name\trows\tloom_us\tvortex_us");

    let bitpack = make_bitpack_fixture();
    report_i32("bitpack-i32-4096", &bitpack);

    let fsst = make_fsst_fixture();
    report_utf8("fsst-utf8-1024", &fsst);
}

fn report_i32(name: &str, fixture: &I32Fixture) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let loom = time(|| {
        black_box(decode_layout_to_array_data(&fixture.desc, &registry).expect("loom decode"));
    });
    let vortex = time(|| {
        black_box(oracle::decode_i32_oracle(&fixture.array));
    });
    println!(
        "{name}\t{}\t{}\t{}",
        fixture.desc.row_count,
        loom.as_micros(),
        vortex.as_micros()
    );
}

fn report_utf8(name: &str, fixture: &Utf8Fixture) {
    let registry = L2KernelRegistry::default_for_mvp0();
    let loom = time(|| {
        black_box(decode_layout_to_array_data(&fixture.desc, &registry).expect("loom decode"));
    });
    let vortex = time(|| {
        black_box(oracle::decode_utf8_oracle(&fixture.array));
    });
    println!(
        "{name}\t{}\t{}\t{}",
        fixture.desc.row_count,
        loom.as_micros(),
        vortex.as_micros()
    );
}

fn time(mut f: impl FnMut()) -> Duration {
    let start = Instant::now();
    f();
    start.elapsed()
}

struct I32Fixture {
    desc: LayoutDescription,
    array: vortex_array::ArrayRef,
}

struct Utf8Fixture {
    desc: LayoutDescription,
    array: vortex_array::ArrayRef,
}

fn make_bitpack_fixture() -> I32Fixture {
    let values = (0i32..4096).map(|v| v % 2048).collect::<Vec<_>>();
    let input = PrimitiveArray::from_iter(values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let packed = BitPackedData::encode(&input.into_array(), 11, &mut ctx).expect("bitpack encode");
    let array = packed.as_array().clone();
    let desc = LayoutDescription {
        data_type: DataType::Int32,
        root: vortex_reader::from_bitpacked_array(&packed),
        row_count: 4096,
    };
    I32Fixture { desc, array }
}

fn make_fsst_fixture() -> Utf8Fixture {
    let rows = (0..1024)
        .map(|i| Some(format!("phase7-string-{i:04}")))
        .collect::<Vec<_>>();
    let refs = rows
        .iter()
        .map(|value| value.as_deref())
        .collect::<Vec<_>>();
    let values = VarBinArray::from_iter(refs, DType::Utf8(Nullability::Nullable));
    let compressor = vortex_fsst::fsst_train_compressor(&values);
    let mut ctx = LEGACY_SESSION.create_execution_ctx();
    let fsst =
        vortex_fsst::fsst_compress(&values, values.len(), values.dtype(), &compressor, &mut ctx);
    let array = fsst.as_array().clone();
    let desc = LayoutDescription {
        data_type: DataType::Utf8,
        root: vortex_reader::from_fsst_array(&fsst),
        row_count: 1024,
    };
    Utf8Fixture { desc, array }
}
