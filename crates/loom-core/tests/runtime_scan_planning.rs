use loom_core::runtime_abi::{
    plan_predicate, plan_projection, plan_split, ConcurrencyPolicy, PredicateEnvelope,
    PredicateOperator, ProjectionColumn, ProjectionSet, ScanShape, SplitDescriptor,
    UnsupportedPredicatePolicy,
};

#[test]
fn projection_all_maps_output_to_source_columns() {
    let plan = plan_projection(&ProjectionSet::All, 3).expect("projection all");
    assert_eq!(plan.output_to_source, vec![0, 1, 2]);
}

#[test]
fn projection_columns_preserves_explicit_output_order() {
    let projection = ProjectionSet::Columns(vec![
        ProjectionColumn {
            source_index: 2,
            output_index: 0,
        },
        ProjectionColumn {
            source_index: 0,
            output_index: 1,
        },
    ]);
    let plan = plan_projection(&projection, 3).expect("projection");
    assert_eq!(plan.output_to_source, vec![2, 0]);
}

#[test]
fn projection_rejects_missing_and_duplicate_columns() {
    let err = plan_projection(
        &ProjectionSet::Columns(vec![ProjectionColumn {
            source_index: 4,
            output_index: 0,
        }]),
        2,
    )
    .expect_err("missing source should reject");
    assert_eq!(err.code.as_str(), "unsupported-projection");

    let err = plan_projection(
        &ProjectionSet::Columns(vec![
            ProjectionColumn {
                source_index: 0,
                output_index: 0,
            },
            ProjectionColumn {
                source_index: 1,
                output_index: 0,
            },
        ]),
        2,
    )
    .expect_err("duplicate output should reject");
    assert_eq!(err.code.as_str(), "unsupported-projection");
}

#[test]
fn predicate_none_is_supported_and_comparison_is_policy_controlled() {
    assert!(plan_predicate(
        &PredicateEnvelope::None,
        UnsupportedPredicatePolicy::FailClosed
    )
    .expect("none predicate"));

    let predicate = PredicateEnvelope::PrimitiveComparison {
        column_index: 0,
        op: PredicateOperator::GtEq,
        literal_i64: 10,
    };
    let err = plan_predicate(&predicate, UnsupportedPredicatePolicy::FailClosed)
        .expect_err("predicate pushdown not implemented");
    assert_eq!(err.code.as_str(), "unsupported-predicate");

    assert!(
        !plan_predicate(&predicate, UnsupportedPredicatePolicy::ScanAll)
            .expect("scan-all fallback")
    );
}

#[test]
fn split_planning_accepts_full_and_row_range_shapes() {
    let shape = ScanShape {
        column_count: 2,
        row_count: 100,
        splittable: true,
    };
    assert_eq!(
        plan_split(
            SplitDescriptor::FullScan { row_count: 100 },
            shape,
            ConcurrencyPolicy::SingleWorker
        )
        .expect("full scan"),
        SplitDescriptor::FullScan { row_count: 100 }
    );
    assert_eq!(
        plan_split(
            SplitDescriptor::RowRange { start: 10, end: 20 },
            shape,
            ConcurrencyPolicy::ParallelSplits {
                requested_workers: 2
            }
        )
        .expect("range split"),
        SplitDescriptor::RowRange { start: 10, end: 20 }
    );
}

#[test]
fn split_planning_rejects_invalid_or_unsafe_concurrency() {
    let shape = ScanShape {
        column_count: 1,
        row_count: 10,
        splittable: false,
    };
    let err = plan_split(
        SplitDescriptor::RowRange { start: 9, end: 11 },
        shape,
        ConcurrencyPolicy::SingleWorker,
    )
    .expect_err("range exceeds row count");
    assert_eq!(err.code.as_str(), "invalid-split");

    let err = plan_split(
        SplitDescriptor::FullScan { row_count: 10 },
        shape,
        ConcurrencyPolicy::ParallelSplits {
            requested_workers: 2,
        },
    )
    .expect_err("non-splittable parallel request");
    assert_eq!(err.code.as_str(), "unsafe-concurrency");
}
