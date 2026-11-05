use bc_core::{
    align::{AlignInput, QuickAlign, SmartAlign},
    cancel::CancelCheck,
    diff::MyersDiff,
    hash::ContentHash,
    CoreError,
};
use bc_jobs::CancelToken;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn token_bridge_cancels_both_aligners_and_diff() {
    let token = CancelToken::new();
    assert!(!CancelCheck::is_cancelled(&token));
    token.cancel();
    assert!(matches!(
        QuickAlign.align2_cancellable(
            AlignInput::default(),
            AlignInput::default(),
            &Default::default(),
            &token
        ),
        Err(CoreError::Cancelled)
    ));
    assert!(matches!(
        SmartAlign::default().align2_cancellable(
            AlignInput::default(),
            AlignInput::default(),
            &Default::default(),
            &token
        ),
        Err(CoreError::Cancelled)
    ));
    assert!(matches!(
        MyersDiff::default().diff_cancellable(&[], &[], &token),
        Err(CoreError::Cancelled)
    ));
}
#[test]
fn prefix_frontier_block_and_expansion_poll_cancel() {
    for mode in 0..3 {
        let token = CancelToken::new();
        let a: Vec<_> = (0..50_000)
            .map(|i| ContentHash(if mode == 1 { i % 31 } else { i }))
            .collect();
        let b: Vec<_> = if mode == 0 {
            a.clone()
        } else {
            a.iter().rev().copied().collect()
        };
        let polls = AtomicUsize::new(0);
        let check = || {
            if polls.fetch_add(1, Ordering::SeqCst) >= 8 {
                token.cancel();
            }
            CancelCheck::is_cancelled(&token)
        };
        assert!(matches!(
            QuickAlign.align2_cancellable(
                AlignInput {
                    hashes: &a,
                    ..Default::default()
                },
                AlignInput {
                    hashes: &b,
                    ..Default::default()
                },
                &Default::default(),
                &check
            ),
            Err(CoreError::Cancelled)
        ));
        assert!(polls.load(Ordering::SeqCst) >= 9);
    }
}
