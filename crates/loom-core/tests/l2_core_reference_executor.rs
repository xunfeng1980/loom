use arrow_schema::DataType;
use loom_core::l2_core::{
    Capability, InputSliceCapability, L2CoreProgram, L2CoreStmt, OutputBuilderCapability,
    ResourceBudget, ScalarExpr, ScalarValue,
};
use loom_core::l2_core_reference_executor::{execute_reference, ReferenceStatus};

fn sample_program() -> L2CoreProgram {
    L2CoreProgram {
        artifact_version: 1,
        required_features: vec!["l2core.copy.v0".to_string()],
        optional_features: vec![],
        capabilities: vec![
            Capability::InputSlice(InputSliceCapability {
                id: "input0".to_string(),
                offset: 0,
                length: 16,
            }),
            Capability::OutputBuilder(OutputBuilderCapability {
                id: "out0".to_string(),
                arrow_type: DataType::Int32,
                nullable: true,
                max_events: 4,
            }),
        ],
        resource_budget: ResourceBudget::bounded_rows(4),
        body: vec![L2CoreStmt::ForRange {
            index: "i".to_string(),
            start: ScalarExpr::u64(0),
            end: ScalarExpr::u64(4),
            body: vec![
                L2CoreStmt::ReadInput {
                    capability: "input0".to_string(),
                    offset: ScalarExpr::Add(
                        Box::new(ScalarExpr::var("i")),
                        Box::new(ScalarExpr::u64(0)),
                    ),
                    width: ScalarExpr::u64(4),
                    bind: "value".to_string(),
                },
                L2CoreStmt::AppendValue {
                    builder: "out0".to_string(),
                    value: ScalarExpr::var("value"),
                },
            ],
        }],
    }
}

fn trace_lines(program: &L2CoreProgram) -> Vec<String> {
    execute_reference(program).trace_lines()
}

#[test]
fn reference_oracle_emits_trace_for_matrix_accepted_copy() {
    let report = execute_reference(&sample_program());

    assert_eq!(report.status, ReferenceStatus::Finished);
    assert_eq!(
        report.trace_lines(),
        vec![
            "read:input0:offset=expr:width=4:in-bounds=true",
            "append-value:out0:int32",
            "terminal:finished",
        ]
    );
}

#[test]
fn reference_oracle_emits_append_null_trace() {
    let mut program = sample_program();
    program.body = vec![L2CoreStmt::AppendNull {
        builder: "out0".to_string(),
    }];

    assert_eq!(
        trace_lines(&program),
        vec!["append-null:out0:int32", "terminal:finished"]
    );
}

#[test]
fn reference_oracle_emits_fail_closed_for_negative_matrix() {
    let mut missing_input = sample_program();
    missing_input
        .capabilities
        .retain(|capability| !matches!(capability, Capability::InputSlice(_)));
    assert_eq!(
        trace_lines(&missing_input),
        vec!["fail-closed:missing-input-capability"]
    );

    let mut missing_output = sample_program();
    missing_output.body = vec![L2CoreStmt::AppendNull {
        builder: "missing".to_string(),
    }];
    assert_eq!(
        trace_lines(&missing_output),
        vec!["fail-closed:missing-output-builder"]
    );

    let mut invalid_loop = sample_program();
    invalid_loop.body = vec![L2CoreStmt::ForRange {
        index: "i".to_string(),
        start: ScalarExpr::u64(4),
        end: ScalarExpr::u64(0),
        body: vec![],
    }];
    assert_eq!(
        trace_lines(&invalid_loop),
        vec!["fail-closed:invalid-loop-bounds"]
    );

    let mut non_monotone = sample_program();
    non_monotone.body = vec![L2CoreStmt::CursorLoop {
        cursor: "cursor".to_string(),
        limit: ScalarExpr::u64(4),
        progress: ScalarExpr::var("cursor"),
        body: vec![],
    }];
    assert_eq!(
        trace_lines(&non_monotone),
        vec!["fail-closed:non-monotone-cursor-loop"]
    );
}

#[test]
fn reference_oracle_emits_trace_for_fuzz_cases() {
    let mut fuzz_000 = sample_program();
    fuzz_000.body = vec![
        L2CoreStmt::LetScalar {
            name: "x".to_string(),
            expr: ScalarExpr::Const(ScalarValue::Int32(7)),
        },
        L2CoreStmt::LetScalar {
            name: "y".to_string(),
            expr: ScalarExpr::Add(
                Box::new(ScalarExpr::var("x")),
                Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            ),
        },
        L2CoreStmt::AppendValue {
            builder: "out0".to_string(),
            value: ScalarExpr::var("y"),
        },
    ];
    assert_eq!(
        trace_lines(&fuzz_000),
        vec!["append-value:out0:int32", "terminal:finished"]
    );

    let mut fuzz_001 = sample_program();
    fuzz_001.capabilities = vec![
        Capability::InputSlice(InputSliceCapability {
            id: "input0".to_string(),
            offset: 0,
            length: 16,
        }),
        Capability::OutputBuilder(OutputBuilderCapability {
            id: "out0".to_string(),
            arrow_type: DataType::Boolean,
            nullable: false,
            max_events: 4,
        }),
    ];
    fuzz_001.body = vec![L2CoreStmt::AppendValue {
        builder: "out0".to_string(),
        value: ScalarExpr::Eq(
            Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
            Box::new(ScalarExpr::Const(ScalarValue::Int32(1))),
        ),
    }];
    assert_eq!(
        trace_lines(&fuzz_001),
        vec!["append-value:out0:bool", "terminal:finished"]
    );

    let mut fuzz_002 = sample_program();
    fuzz_002.capabilities = vec![Capability::OutputBuilder(OutputBuilderCapability {
        id: "score32".to_string(),
        arrow_type: DataType::Float32,
        nullable: false,
        max_events: 4,
    })];
    fuzz_002.body = vec![L2CoreStmt::AppendValue {
        builder: "score32".to_string(),
        value: ScalarExpr::Const(ScalarValue::Float32Bits(1.5f32.to_bits())),
    }];
    assert_eq!(
        trace_lines(&fuzz_002),
        vec!["append-value:score32:float32", "terminal:finished"]
    );

    let mut fuzz_003 = sample_program();
    fuzz_003.capabilities = vec![Capability::OutputBuilder(OutputBuilderCapability {
        id: "score64".to_string(),
        arrow_type: DataType::Float64,
        nullable: true,
        max_events: 4,
    })];
    fuzz_003.body = vec![
        L2CoreStmt::AppendValue {
            builder: "score64".to_string(),
            value: ScalarExpr::Const(ScalarValue::Float64Bits((-2.25f64).to_bits())),
        },
        L2CoreStmt::AppendNull {
            builder: "score64".to_string(),
        },
    ];
    assert_eq!(
        trace_lines(&fuzz_003),
        vec![
            "append-value:score64:float64",
            "append-null:score64:float64",
            "terminal:finished"
        ]
    );
}
