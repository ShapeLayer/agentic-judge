"""Create a starter problem once, preserving existing study data."""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def call(tool, **arguments):
    result = subprocess.run(
        [str(ROOT / "target/debug/agentic-judge"), "call", "--config",
         str(ROOT / "config/study.toml"), tool,
         json.dumps(arguments, ensure_ascii=False)],
        check=True, capture_output=True, text=True,
    )
    return json.loads(result.stdout)


def main():
    problem_id = "study-sum"
    if (ROOT / ".study/data/problems" / problem_id).exists():
        return
    call("problem_create", problem_id=problem_id, name="첫 연습: A+B")
    for section, markdown in {
        "legend": "두 정수 A와 B를 입력받아 합을 출력하세요.",
        "input": "첫 줄에 정수 A와 B가 공백으로 주어집니다. -10⁹ ≤ A, B ≤ 10⁹.",
        "output": "A+B를 출력하세요.",
    }.items():
        call("statement_set", problem_id=problem_id, section=section, markdown=markdown)
    call("example_set", problem_id=problem_id, number=1, input="2 3\n", output="5\n")
    call("answer_save", problem_id=problem_id, language="python",
         source="a, b = map(int, input().split())\nprint(a + b)\n", expected="AC", main=True)
    for number, data in enumerate(["2 3\n", "0 0\n", "-7 3\n", "1000000000 1000000000\n"], 1):
        call("test_put", problem_id=problem_id, suite="general", number=number, input=data)
    print("A+B 연습 문제와 채점 테스트 4개를 준비했습니다.")


if __name__ == "__main__":
    main()
