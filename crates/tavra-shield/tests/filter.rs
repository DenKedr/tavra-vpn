use tavra_shield::*;
#[test]
fn suffix_boundaries_allow_override_categories_and_counts() {
    let compiled = CompiledRules::compile(&[
        Source {
            category: Category::Ads,
            text: "||example.com^\n@@||safe.example.com^\n0.0.0.0 exact.test",
        },
        Source {
            category: Category::Malware,
            text: "||danger.test^",
        },
    ])
    .unwrap();
    let engine = Engine::new(compiled, Policy::default());
    assert_eq!(
        engine.check("WWW.Example.COM.").unwrap(),
        Decision::Block(Category::Ads)
    );
    for host in [
        "evil-example.com",
        "example.com.evil.test",
        "safe.example.com",
        "x.safe.example.com",
        "x.exact.test",
    ] {
        assert_eq!(engine.check(host).unwrap(), Decision::Allow);
    }
    assert_eq!(
        engine.check("exact.test").unwrap(),
        Decision::Block(Category::Ads)
    );
    assert_eq!(engine.counters().blocked, 2);
    assert_eq!(engine.counters().allowed, 5);
    let disabled = Engine::new(
        CompiledRules::compile(&[Source {
            category: Category::Ads,
            text: "||example.com^",
        }])
        .unwrap(),
        Policy {
            ads: false,
            ..Policy::default()
        },
    );
    assert_eq!(disabled.check("example.com").unwrap(), Decision::Allow);
}
#[test]
fn rejects_invalid_catchall_and_reports_unsupported_without_activating_them() {
    let c = CompiledRules::compile(&[Source { category: Category::Trackers, text: "||*^\n||com^\n||-bad.test^\n||bad..test^\nexample.test##.ad\n/foo/\n||ads.test^$important\n||valid.test^" }]).unwrap();
    assert_eq!(c.report().accepted, 1);
    assert_eq!(c.report().rejected.len(), 7);
    let e = Engine::new(c, Policy::default());
    for host in [
        "",
        "*",
        "bad..test",
        "a/b.test",
        "127.0.0.1",
        "https://example.test",
        "bad_test.test",
        "a.test..",
    ] {
        assert!(e.check(host).is_err(), "{host}");
    }
    assert_eq!(e.counters().allowed, 0);
    assert!(
        CompiledRules::compile(&[Source {
            category: Category::Ads,
            text: &"a".repeat(MAX_SOURCE_BYTES + 1)
        }])
        .is_err()
    );
}
#[test]
fn idna_and_exact_rules_are_canonical() {
    let c = CompiledRules::compile(&[Source {
        category: Category::Annoyances,
        text: "||BÜCHER.example^\nexact:only.test\nallow:only.test",
    }])
    .unwrap();
    let e = Engine::new(c, Policy::default());
    assert_eq!(
        e.check("xn--bcher-kva.example").unwrap(),
        Decision::Block(Category::Annoyances)
    );
    assert_eq!(e.check("only.test").unwrap(), Decision::Allow);
    assert_eq!(e.check("sub.only.test").unwrap(), Decision::Allow);
}
#[test]
fn category_priority_and_atomic_compilation_bounds() {
    let c = CompiledRules::compile(&[
        Source {
            category: Category::Ads,
            text: "||same.test^",
        },
        Source {
            category: Category::Malware,
            text: "||same.test^",
        },
    ])
    .unwrap();
    let e = Engine::new(c, Policy::default());
    assert_eq!(
        e.check("same.test").unwrap(),
        Decision::Block(Category::Malware)
    );
    assert_eq!(e.counters().malware, 1);
    assert_eq!(e.counters().ads, 0);
    let too_many = "||valid.test^\n".repeat(MAX_RULES + 1);
    assert!(
        CompiledRules::compile(&[Source {
            category: Category::Ads,
            text: &too_many
        }])
        .is_err()
    );
}
