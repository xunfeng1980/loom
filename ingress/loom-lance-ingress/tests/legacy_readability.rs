use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use arrow_array::{
    Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch, RecordBatchIterator,
};
use arrow_schema::{DataType, Field, Schema};
use lance::Dataset;
use loom_core::arrow_semantic_codec::decode_arrow_semantic_payload;
use loom_core::artifact_verifier::{verify_artifact, ArtifactVerificationStatus};
use loom_core::container_codec::decode_table_payload_maybe_container;
use loom_core::l2_kernel_registry::L2KernelRegistry;
use loom_core::table_codec::decode_table_to_array_data;
use loom_lance_ingress::lance_source_facts_from_path;
use tempfile::TempDir;

use futures::TryStreamExt;
use loom_core::arrow_semantic::{ArrowSemanticBatch, ArrowSemanticPayload};
use loom_core::arrow_semantic_codec::encode_arrow_semantic_payload;
use loom_source_ingress::{
    SourceArtifactVerificationSummary, SourceDiagnostic, SourceDiagnosticCode,
    SourceEmissionDisposition, SourceEmissionKind, SourceIngressAcceptedArtifact,
    SourceIngressReport, SourceIngressStatus, SourceLoweringDisposition, SourceOracleEvidence,
    SourceOracleStrategy,
};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("legacy")
}

/// Dev-time oracle: read Arrow batches from a Lance dataset.
async fn dev_time_lance_oracle(path: &Path) -> Result<Vec<RecordBatch>, String> {
    let dataset = Dataset::open(path.to_str().ok_or("non-utf8 path")?)
        .await.map_err(|e| format!("open: {e}"))?;
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await.map_err(|e| format!("scan: {e}"))?;
    stream.try_collect::<Vec<_>>().await.map_err(|e| format!("collect: {e}"))
}

/// Dev-time LMA1 emission: replicates old dev_time_emit_lma1.
async fn dev_time_emit_lma1(path: &Path) -> Result<SourceIngressAcceptedArtifact, SourceIngressReport> {
    let source_facts = lance_source_facts_from_path(path).await?;
    let batches = dev_time_lance_oracle(path).await.map_err(|msg| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", msg))
    })?;
    let schema = batches.first().map(RecordBatch::schema).ok_or_else(|| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", "no batches"))
    })?;
    let semantic = batches.iter().map(ArrowSemanticBatch::from_record_batch)
        .collect::<Result<Vec<_>, _>>().map_err(|err| {
            SourceIngressReport::unsupported(Some(source_facts.clone()),
                SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("batch: {err}")))
        })?;
    let payload = ArrowSemanticPayload::try_new(schema, semantic).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("payload: {err}")))
    })?;
    let lma1_bytes = encode_arrow_semantic_payload(&payload).map_err(|err| {
        SourceIngressReport::unsupported(Some(source_facts.clone()),
            SourceDiagnostic::new(SourceDiagnosticCode::UnsupportedConversion, "$.oracle", format!("LMA1: {err}")))
    })?;
    let row_count = batches.iter().map(|b| b.num_rows() as u64).sum();
    let mut oracle = SourceOracleEvidence::accepted(SourceOracleStrategy::SourceNativeScan, row_count);
    oracle.nulls_checked = true;
    oracle.notes.push("dev-time Lance source-native oracle evidence only".to_string());
    let artifact_summary = SourceArtifactVerificationSummary::accepted(lma1_bytes.len(), "LMA1 payload accepted".to_string());
    let report = SourceIngressReport::accepted(source_facts, SourceEmissionKind::ArrowSemantic,
        SourceEmissionDisposition::SemanticArrow, SourceLoweringDisposition::InterpreterOnly,
        artifact_summary, oracle).expect("accepted");
    Ok(SourceIngressAcceptedArtifact { bytes: lma1_bytes, report })
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn shasum");
    child
        .stdin
        .as_mut()
        .expect("shasum stdin")
        .write_all(bytes)
        .expect("write bytes to shasum");
    let output = child.wait_with_output().expect("read shasum output");
    assert!(output.status.success(), "shasum failed");
    String::from_utf8(output.stdout)
        .expect("utf8 shasum output")
        .split_whitespace()
        .next()
        .expect("sha256 digest")
        .to_string()
}

fn sha256_file(path: &Path) -> String {
    sha256_bytes(&std::fs::read(path).expect("read fixture"))
}

fn collect_tree_bytes(root: &Path, current: &Path, out: &mut Vec<u8>) {
    let mut entries = std::fs::read_dir(current)
        .expect("read tree")
        .map(|entry| entry.expect("tree entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_tree_bytes(root, &path, out);
        } else {
            let relative = path.strip_prefix(root).expect("relative path");
            out.extend(relative.to_string_lossy().as_bytes());
            out.push(0);
            out.extend(std::fs::read(&path).expect("read tree file"));
            out.push(0);
        }
    }
}

fn sha256_tree(root: &Path) -> String {
    let mut bytes = Vec::new();
    collect_tree_bytes(root, root, &mut bytes);
    sha256_bytes(&bytes)
}

fn expected_batch() -> RecordBatch {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("score", DataType::Int64, false),
        Field::new("ratio32", DataType::Float32, false),
        Field::new("ratio64", DataType::Float64, false),
    ]));
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3])),
            Arc::new(Int64Array::from(vec![10, 20, 30])),
            Arc::new(Float32Array::from(vec![1.25, -2.5, 3.75])),
            Arc::new(Float64Array::from(vec![1.5, 2.5, 3.5])),
        ],
    )
    .expect("expected batch")
}

fn assert_batch_matches_expected(batch: &RecordBatch) {
    assert_eq!(batch.num_rows(), 3);
    assert_eq!(batch.num_columns(), 4);
    let ids = batch
        .column(0)
        .as_any()
        .downcast_ref::<Int32Array>()
        .expect("id Int32");
    let scores = batch
        .column(1)
        .as_any()
        .downcast_ref::<Int64Array>()
        .expect("score Int64");
    let ratio32 = batch
        .column(2)
        .as_any()
        .downcast_ref::<Float32Array>()
        .expect("ratio32 Float32");
    let ratio64 = batch
        .column(3)
        .as_any()
        .downcast_ref::<Float64Array>()
        .expect("ratio64 Float64");
    assert_eq!(ids.null_count(), 0);
    assert_eq!(scores.null_count(), 0);
    assert_eq!(ratio32.null_count(), 0);
    assert_eq!(ratio64.null_count(), 0);
    assert_eq!(
        (0..ids.len()).map(|idx| ids.value(idx)).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        (0..scores.len())
            .map(|idx| scores.value(idx))
            .collect::<Vec<_>>(),
        vec![10, 20, 30]
    );
    assert_eq!(
        (0..ratio32.len())
            .map(|idx| ratio32.value(idx))
            .collect::<Vec<_>>(),
        vec![1.25, -2.5, 3.75]
    );
    assert_eq!(
        (0..ratio64.len())
            .map(|idx| ratio64.value(idx))
            .collect::<Vec<_>>(),
        vec![1.5, 2.5, 3.5]
    );
}

fn decode_loom_batch(bytes: &[u8]) -> RecordBatch {
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    let table = decode_table_payload_maybe_container(bytes).expect("decode legacy LMT1");
    assert_eq!(table.row_count, 3);
    assert_eq!(
        table
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        vec!["id", "score", "ratio32", "ratio64"]
    );
    let arrays = decode_table_to_array_data(&table, &registry).expect("decode table arrays");
    RecordBatch::try_new(
        expected_batch().schema(),
        vec![
            Arc::new(Int32Array::from(arrays[0].clone())),
            Arc::new(Int64Array::from(arrays[1].clone())),
            Arc::new(Float32Array::from(arrays[2].clone())),
            Arc::new(Float64Array::from(arrays[3].clone())),
        ],
    )
    .expect("decoded legacy batch")
}

async fn write_current_lance(path: &Path, batch: RecordBatch) {
    let schema = batch.schema();
    let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);
    Dataset::write(reader, path.to_str().expect("utf-8 temp path"), None)
        .await
        .expect("write current Lance rewrite");
}

#[tokio::test(flavor = "current_thread")]
async fn legacy_lance_fixture_has_paired_verifier_accepted_loom_and_current_rewrite_proof() {
    let dir = fixture_dir();
    let source = dir.join("legacy-v1.lance");
    let loom = dir.join("legacy-v1.loom");
    let manifest = dir.join("MANIFEST.md");

    assert!(
        source.is_dir(),
        "actual older-version Lance dataset fixture directory is required"
    );
    assert!(loom.is_file(), "paired legacy Loom artifact is required");
    assert!(manifest.is_file(), "legacy fixture manifest is required");

    let manifest_text = std::fs::read_to_string(&manifest).expect("read manifest");
    assert!(manifest_text.contains("source_family: lance"));
    assert!(manifest_text.contains("generator_crate: lance"));
    assert!(manifest_text.contains("generator_version: 6.0.0"));
    assert!(manifest_text.contains("schema: id:Int32 non-null, score:Int64 non-null, ratio32:Float32 non-null, ratio64:Float64 non-null"));
    assert!(manifest_text.contains("rows: [(1,10,1.25,1.5), (2,20,-2.5,2.5), (3,30,3.75,3.5)]"));
    assert!(manifest_text.contains("current_rewrite_proof: current loom-lance-ingress"));
    assert!(manifest_text.contains("emit_source_ingress_lma1_from_lance_path"));

    let source_hash = sha256_tree(&source);
    let loom_hash = sha256_file(&loom);
    assert!(manifest_text.contains(&format!("source_fixture_tree_sha256: {source_hash}")));
    assert!(manifest_text.contains(&format!("paired_loom_sha256: {loom_hash}")));

    let paired_batch = decode_loom_batch(&std::fs::read(&loom).expect("read paired loom"));
    assert_batch_matches_expected(&paired_batch);

    let source_batches = dev_time_lance_oracle(&source)
        .await
        .expect("current Lance reader reads actual older-version fixture");
    assert_eq!(source_batches.len(), 1);
    assert_batch_matches_expected(&source_batches[0]);

    let accepted = dev_time_emit_lma1(&source)
        .await
        .expect("current Lance adapter emits verifier-accepted Loom from older fixture");
    let registry = L2KernelRegistry::default_for_mvp0();
    let report = verify_artifact(&accepted.bytes, &registry, &Default::default());
    assert_eq!(report.status(), ArtifactVerificationStatus::Accepted);
    assert_eq!(
        report.facts().expect("accepted LMA1 facts").artifact_kind,
        "LMA1"
    );
    let semantic_batches = decode_arrow_semantic_payload(&accepted.bytes)
        .expect("decode current direct LMA1")
        .to_record_batches()
        .expect("decode current direct LMA1 batches");
    assert_eq!(semantic_batches, source_batches);

    let temp = TempDir::new().expect("tempdir");
    let rewritten = temp.path().join("legacy-current-rewrite.lance");
    write_current_lance(&rewritten, paired_batch).await;
    let rewritten_batches = dev_time_lance_oracle(&rewritten)
        .await
        .expect("read current rewrite");
    assert_eq!(rewritten_batches.len(), 1);
    assert_batch_matches_expected(&rewritten_batches[0]);
}
