//! `score` and `abstain`: the two fields the risk-coverage curve needs.
//!
//! A score orders abstention without changing admission; an abstention names objects the candidate
//! declines to classify. Both are optional, and a response without them must keep its old verdict
//! byte for byte.

use metaform::propose::validate_response;

fn request() -> &'static str {
    r#"{"protocol":"metaform.propose/1","request_id":"r-abstain","objects":4,
        "object_ids":[0,1,2,3],
        "vocabulary":[{"name":"a","ids":[0]}],
        "evidence":{"pos":[0],"neg":[1]},
        "allowed":{"ids":true,"define":{"ops":["all_of","any_of"],"operands":"vocabulary"}}}"#
}

fn verdict(response: &str) -> String {
    format!("{:?}", validate_response(request(), response).expect("request parses"))
}

#[test]
fn a_score_and_an_abstention_are_echoed() {
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-abstain","refused":null,
            "candidates":[{"name":"c","ids":[0],"score":0.5,"abstain":[2],
                           "predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("score: Some(0.5)"), "score must be kept: {out}");
    assert!(out.contains("abstain: [2]"), "abstention must be kept: {out}");
    assert!(!out.contains("rejected: [Rejected"), "the candidate must be accepted: {out}");
}

#[test]
fn a_score_outside_the_unit_interval_is_refused() {
    for bad in ["1.5", "-0.1", "\"high\""] {
        let out = verdict(&format!(
            r#"{{"protocol":"metaform.propose/1","request_id":"r-abstain",
                 "candidates":[{{"name":"c","ids":[0],"score":{bad},
                                "predict":{{"id":2,"label":"pos"}}}}]}}"#
        ));
        assert!(out.contains("bad_score"), "score {bad} must be refused: {out}");
    }
}

#[test]
fn abstaining_on_a_measured_object_is_refused() {
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-abstain",
            "candidates":[{"name":"c","ids":[0],"abstain":[1],
                           "predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("abstain_measured_object"), "{out}");
}

#[test]
fn abstaining_on_an_object_the_candidate_claims_is_refused() {
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-abstain",
            "candidates":[{"name":"c","ids":[3],"abstain":[3],"predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("abstain_overlaps_extension"), "{out}");
}

#[test]
fn abstaining_without_a_score_is_refused() {
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-abstain",
            "candidates":[{"name":"c","ids":[0],"abstain":[3],"predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("missing_score_for_abstention"), "{out}");
}
#[test]
fn a_response_without_the_new_fields_is_unchanged() {
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-abstain",
            "candidates":[{"name":"c","ids":[0],"predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("score: None"), "{out}");
    assert!(out.contains("abstain: []"), "{out}");
    assert!(!out.contains("rejected: [Rejected"), "{out}");
}
