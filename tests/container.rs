use agentic_judge::{model::Config, runtime::Runtime};
#[test]
#[ignore = "requires Podman and the prebuilt localhost/agentic-judge-runtime:local image"]
fn podman_limits_and_languages() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Runtime {
        config: Config {
            runtime_root: dir.path().into(),
            ..Config::default()
        },
    };
    let snippets = [
        ("cpp", "#include <iostream>\nint main(){std::cout<<42;}"),
        ("c", "#include <stdio.h>\nint main(){printf(\"42\");}"),
        ("rust", "fn main(){println!(\"42\");}"),
        ("python", "print(42)"),
        ("javascript", "console.log(42)"),
        ("typescript", "console.log(42)"),
        (
            "java",
            "public class Main {public static void main(String[] a){System.out.println(42);}}",
        ),
        (
            "go",
            "package main\nimport \"fmt\"\nfunc main(){fmt.Println(42)}",
        ),
        ("ruby", "puts 42"),
        ("php", "<?php echo 42;"),
        ("bash", "echo 42"),
    ];
    for (language, source) in snippets {
        let p = rt
            .prepare(language, source)
            .unwrap_or_else(|e| panic!("{language}: {e}"));
        let r = rt.run(&p, "", &[], 10000, 512).unwrap();
        assert_eq!(r.verdict, "AC", "{language}: {}", r.stderr);
        assert_eq!(r.stdout.trim(), "42");
    }
    let p = rt
        .prepare("python", "x=bytearray(256*1024*1024); print(len(x))")
        .unwrap();
    assert_eq!(rt.run(&p, "", &[], 10000, 64).unwrap().verdict, "MLE");
    let p = rt.prepare("python", "while True: pass").unwrap();
    assert_eq!(rt.run(&p, "", &[], 2000, 128).unwrap().verdict, "TLE");
    let p=rt.prepare("python","import socket\ns=socket.socket();s.settimeout(1)\ntry:\n s.connect(('1.1.1.1',80))\n raise AssertionError('network accessible')\nexcept OSError:\n print('isolated')").unwrap();
    assert_eq!(
        rt.run(&p, "", &[], 10000, 128).unwrap().stdout.trim(),
        "isolated"
    );
}
