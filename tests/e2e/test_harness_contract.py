import re
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


E2E_ROOT = Path(__file__).resolve().parent
SOURCED_SUITE_FILES = [
    E2E_ROOT / "lib.sh",
    *sorted((E2E_ROOT / "scenarios").glob("*.sh")),
]

def shell_function_body(path: Path, name: str) -> str:
    match = re.search(rf"^{name}\(\) \{{(?P<body>.*?)\n\}}", path.read_text(), re.DOTALL | re.MULTILINE)
    if match is None:
        raise AssertionError(f"{name} is not defined in {path.name}")
    return match.group("body")


class HarnessContractTests(unittest.TestCase):
    def test_sourced_suite_has_one_canonical_exit_path(self):
        exits = []
        abort_calls = []
        for path in SOURCED_SUITE_FILES:
            source = path.read_text()
            for line_number, line in enumerate(source.splitlines(), start=1):
                if re.fullmatch(r"\s*exit 1\s*", line):
                    exits.append((path.name, line_number))
                if re.search(r"\babort_suite\s+\"", line):
                    abort_calls.append((path.name, line_number))

        self.assertEqual(len(exits), 1, exits)
        self.assertEqual(exits[0][0], "lib.sh")
        self.assertTrue(abort_calls)
        source = (E2E_ROOT / "lib.sh").read_text()
        abort = re.search(r"abort_suite\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)
        self.assertIsNotNone(abort)
        body = abort.group("body")
        self.assertIn('badmsg "$1"', body)
        self.assertIn("finish", body)
        self.assertNotIn("cleanup", body)
        self.assertRegex(body, r"(?m)^\s*exit 1\s*$")
        finish = re.search(r"finish\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)
        self.assertIsNotNone(finish)
        self.assertNotIn("abort_suite", finish.group("body"))

    def test_json_parsed_captures_read_stdout_only(self):
        offenders = [
            f"{path.name}:{number}"
            for path in SOURCED_SUITE_FILES
            for number, line in enumerate(path.read_text().splitlines(), 1)
            if '2>&1)"' in line
        ]
        self.assertEqual(
            offenders,
            [],
            "the JSON envelope is on stdout; a capture that merges stderr "
            "breaks json_tool when the binary logs a warning",
        )

    def test_denied_automation_skips_when_notification_center_cannot_be_closed(self):
        source = (E2E_ROOT / "scenarios" / "acceptance.sh").read_text()

        unavailable = source.index(
            "Notification Center could not be proven closed after best-effort preparation"
        )
        permission_gate = source.index(
            'if [ "$(json_field "$nc_probe" error.code)" != "POLICY_DENIED" ]'
        )
        denial_call = source.index('denied_output="$("$bin" --headed list-notifications')
        self.assertLess(permission_gate, unavailable)
        self.assertLess(unavailable, denial_call)
        self.assertIn("skipmsg", source[permission_gate:denial_call])
        self.assertNotIn("badmsg", source[permission_gate:denial_call])

    def test_native_runner_gates_on_strict_headless_non_interference(self):
        source = (E2E_ROOT / "run.sh").read_text()

        safe_gate = source.index('bash "$here/safe-semantic.sh"')
        focused_fixture = source.index("prepare_native_harness")
        self.assertLess(safe_gate, focused_fixture)

    def test_multi_agent_probe_runs_before_the_interaction_lease_is_released(self):
        source = (E2E_ROOT / "run.sh").read_text()
        lease_holder = source.index('exec "$e2e_python" "$here/interaction_lock.py"')
        probe = source.index("cursor-multi-agent-probe.py")
        finishes = [match.start() for match in re.finditer(r"^finish$", source, re.MULTILINE)]

        self.assertEqual(len(finishes), 1)
        self.assertLess(lease_holder, probe)
        self.assertLess(probe, finishes[0])
        for line in source[probe:finishes[0]].splitlines():
            self.assertNotRegex(line, r"\b(exit|exec)\b|release_exclusive_lock")

    def test_native_runner_builds_locked_canonical_artifacts_before_desktop_lock(self):
        source = (E2E_ROOT / "run.sh").read_text()

        cli_build = source.index("cargo build --locked --release -p agent-desktop")
        ffi_build = source.index("cargo build --locked --profile release-ffi")
        desktop_lock = source.index('exec "$e2e_python" "$here/interaction_lock.py"')
        permission_tests = source.index("cargo test --locked --no-run --lib")
        self.assertLess(permission_tests, desktop_lock)
        self.assertIn('export AGENT_DESKTOP_E2E_CARGO_TARGET_DIR="$repo/target"', source)
        self.assertLess(cli_build, desktop_lock)
        self.assertLess(ffi_build, desktop_lock)
        self.assertIn(
            'AGENT_DESKTOP_E2E_RELEASE_BIN="$repo/target/release/agent-desktop"',
            source,
        )
        self.assertIn(
            'AGENT_DESKTOP_E2E_RELEASE_FFI="$repo/target/release-ffi/libagent_desktop_ffi.dylib"',
            source,
        )

    def test_post_launch_readiness_failure_still_closes_owned_fixture(self):
        launch = shell_function_body(E2E_ROOT / "lib.sh", "launch_fixture")
        opened = launch.index('guard_exec 10 1048576 "${open_args[@]}" "$fixture_app"')
        owned = launch.index("fixture_owned=1", opened)
        readiness = launch.index('if [ -z "$ready" ]', owned)
        self.assertLess(opened, owned)
        self.assertLess(owned, readiness)

        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "calls"
            mock_bin = Path(directory) / "agent-desktop"
            mock_bin.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$MOCK_CALLS"\n')
            mock_bin.chmod(0o700)
            script = r'''
source "$1"
bin="$2"
fixture_owned=1
fixture_started=0
cleanup_isolated_environment() { :; }
release_exclusive_lock() { :; }
cleanup
'''
            environment = os.environ.copy()
            environment["MOCK_CALLS"] = str(log)
            subprocess.run(
                [
                    "bash",
                    "-c",
                    script,
                    "cleanup-test",
                    str(E2E_ROOT / "lib.sh"),
                    str(mock_bin),
                ],
                check=True,
                env=environment,
            )

            self.assertEqual(log.read_text().strip(), "close-app AgentDeskFixture --force")

    def test_success_checks_every_staged_native_artifact_before_returning(self):
        source = (E2E_ROOT / "lib.sh").read_text()
        finish = re.search(r"finish\(\) \{(?P<body>.*?)\n\}", source, re.DOTALL)

        self.assertIsNotNone(finish)
        body = finish.group("body")
        self.assertIn('verify_immutable_binary "$raw_bin" "$raw_bin_sha"', body)
        self.assertIn('verify_immutable_binary "$ffi_dylib" "$ffi_dylib_sha"', body)
        self.assertIn('verify_immutable_binary "$ffi_helper" "$ffi_helper_sha"', body)
        decision = body.index('if [ "$fail" -gt 0 ]')
        self.assertLess(body.index('verify_immutable_binary "$ffi_dylib"'), decision)
        self.assertLess(body.index('verify_immutable_binary "$ffi_helper"'), decision)

    def test_cleanup_recycles_owned_artifact_and_leaves_sibling(self):
        with tempfile.TemporaryDirectory() as root:
            root_path = Path(root)
            artifact = root_path / "artifact"
            artifact.mkdir()
            sibling = root_path / "sibling"
            sibling.mkdir()

            payload = artifact / "evidence.txt"
            payload.write_text("keep me")
            result = self.run_cleanup_helper(
                str(artifact), 'trash() { /bin/mv "$1" "$1.recycled"; }'
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(artifact.exists())
            self.assertTrue(sibling.is_dir())
            self.assertEqual(
                (root_path / "artifact.recycled" / "evidence.txt").read_text(), "keep me"
            )

    def test_cleanup_refuses_empty_path(self):
        result = self.run_cleanup_helper("")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("refused an empty path", result.stderr)

    def test_cleanup_retains_artifact_without_a_recycle_backend(self):
        with tempfile.TemporaryDirectory() as root:
            artifact = Path(root) / "artifact"
            artifact.mkdir()
            result = self.run_cleanup_helper(str(artifact), 'PATH="/missing-trash-backend"')
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue(artifact.is_dir())
            self.assertIn("retained artifact", result.stderr)

    def run_cleanup_helper(self, artifact: str, backend: str = ""):
        script = f'''
source "{E2E_ROOT / "harness.sh"}"
{backend}
remove_artifact "{artifact}"
'''
        return subprocess.run(
            ["/bin/bash", "-c", script],
            capture_output=True,
            check=False,
            text=True,
        )


ERROR_CODE_SOURCE = E2E_ROOT.parent.parent / "crates" / "core" / "src" / "response" / "error_code.rs"
ERROR_CONTRACT = E2E_ROOT / "scenarios" / "error_contract.sh"
CONTENTION_GUARD = "check_fixture_contention || { finish; exit 1; }"


def core_error_codes():
    return set(re.findall(r'=> "([A-Z_]+)"', ERROR_CODE_SOURCE.read_text()))


def envelope_calls(source):
    return re.findall(
        r'assert_error_envelope\s+"[^"]*"\s+([A-Z_]+)\s+(\S+)\s+(\S+(?:\s+"")?)',
        source,
    )


class ErrorContractLegTests(unittest.TestCase):
    def test_error_contract_runs_after_surfaces_behind_the_contention_guard(self):
        run_source = (E2E_ROOT / "run.sh").read_text()
        surfaces = run_source.index("source \"$here/scenarios/surfaces.sh\"")
        source_line = 'source "$here/scenarios/error_contract.sh"'
        registered = run_source.index(source_line)
        guard = run_source.rindex(CONTENTION_GUARD, 0, registered)
        self.assertLess(surfaces, guard)
        between = run_source[guard + len(CONTENTION_GUARD) : registered].strip().splitlines()
        self.assertEqual(
            between, ["# shellcheck source=tests/e2e/scenarios/error_contract.sh"]
        )
        self.assertLess(registered, run_source.index('source "$here/scenarios/trace_performance.sh"'))
        self.assertLess(registered, run_source.index("cursor-multi-agent-probe.py"))

    def test_error_contract_codes_exist_in_the_core_enum(self):
        known = core_error_codes()
        self.assertIn("INVALID_ARGS", known)
        source = ERROR_CONTRACT.read_text()
        used = {code for code, _, _ in envelope_calls(source)}
        used |= set(re.findall(r'error\.code\)?"?\s*\]?\s*=\s*"([A-Z_]+)"', source))
        self.assertTrue(used)
        self.assertEqual(used - known, set())

    def test_error_contract_leg_uses_exit_code_and_code_together(self):
        calls = envelope_calls(ERROR_CONTRACT.read_text())
        self.assertGreaterEqual(len(calls), 5)
        for code, exit_code, _ in calls:
            self.assertRegex(exit_code, r"^[12]$", code)
        self.assertIn(("INVALID_ARGS", "2"), {(c, e) for c, e, _ in calls})

    def test_batch_leg_pins_continue_and_stop_semantics(self):
        source = ERROR_CONTRACT.read_text()
        self.assertIn('run_timed "$bin" batch "$payload"\n', source)
        self.assertIn('run_timed "$bin" batch "$payload" --stop-on-error', source)
        self.assertIn("data.stopped.reason", source)
        self.assertIn("data.not_started_entries", source)
        self.assertIn("error.details.batch_index", source)
        self.assertIn('json_field "$(get_target "$field" --property value)" data.value', source)

    def test_batch_malformed_run_uses_a_value_the_field_does_not_already_hold(self):
        source = ERROR_CONTRACT.read_text()
        runtime_first = re.findall(r"runtime (\S+)\)", source)
        malformed_first = re.findall(r"malformed (\S+)\)", source)
        self.assertEqual(len(malformed_first), 1)
        self.assertNotIn(malformed_first[0], runtime_first)
        self.assertNotEqual(malformed_first[0], "batch-last")

    def test_no_scenario_forces_a_blocked_combo(self):
        for path in sorted((E2E_ROOT / "scenarios").glob("*.sh")):
            for number, line in enumerate(path.read_text().splitlines(), start=1):
                if re.search(r"\bpress\b", line) and "--force" in line:
                    self.fail(f"{path.name}:{number} presses with --force")

    def run_helpers(self, body: str):
        with tempfile.TemporaryDirectory() as directory:
            mock_bin = Path(directory) / "agent-desktop"
            mock_bin.write_text(
                "#!/bin/sh\n"
                'printf \'%s\' \'{"ok":false,"error":{"code":"INVALID_ARGS",'
                '"disposition":{"delivery":"not_delivered"}}}\'\n'
                "exit 1\n"
            )
            mock_bin.chmod(0o700)
            script = (
                'source "$1"\n'
                'bin="$2"\n'
                f'json_tool="{E2E_ROOT / "json_tool.py"}"\n'
                "pass=0; fail=0; failures=(); skip=0; skips=()\n"
                "okmsg() { pass=$((pass + 1)); }\n"
                "badmsg() { fail=$((fail + 1)); }\n"
                "sleep() { :; }\n" + body + '\nprintf "pass=%s fail=%s" "$pass" "$fail"\n'
            )
            return subprocess.run(
                ["bash", "-c", script, "helpers", str(E2E_ROOT / "lib.sh"), str(mock_bin)],
                capture_output=True,
                check=True,
                text=True,
            ).stdout

    def test_assert_error_envelope_passes_only_when_every_part_matches(self):
        output = self.run_helpers(
            'assert_error_envelope match INVALID_ARGS 1 not_delivered click\n'
            'assert_error_envelope wrong-code ELEMENT_NOT_FOUND 1 not_delivered click\n'
            'assert_error_envelope wrong-exit INVALID_ARGS 2 not_delivered click\n'
            'assert_error_envelope wrong-delivery INVALID_ARGS 1 delivered click\n'
            'assert_error_envelope delivery-unchecked INVALID_ARGS 1 "" click'
        )
        self.assertTrue(output.endswith("pass=2 fail=3"), output)

    def test_await_value_waits_for_the_expected_readout_without_a_fixed_sleep(self):
        output = self.run_helpers(
            'counter="$(mktemp)"; echo 0 > "$counter"\n'
            'find_value() { n=$(<"$counter"); echo $((n + 1)) > "$counter"; '
            '[ "$n" -ge 3 ] && printf done || printf stale; }\n'
            'await_value readout done >/dev/null && echo reached=yes || echo reached=no\n'
            'find_value() { printf stale; }\n'
            'await_value readout done >/dev/null && echo never=yes || echo never=no'
        )
        self.assertIn("reached=yes", output)
        self.assertIn("never=no", output)


SCENARIOS = E2E_ROOT / "scenarios"
KEYBOARD = SCENARIOS / "keyboard.sh"
REF_CHURN = SCENARIOS / "ref_churn.sh"
RELIABILITY = SCENARIOS / "reliability.sh"
INTERACTION = SCENARIOS / "interaction.sh"


def literal_error_codes(source):
    used = {code for code, _, _ in envelope_calls(source)}
    return used | set(re.findall(r'code"?\s*\]?\s*=\s*"([A-Z_]+)"', source))


class KeyboardAndRefChurnLegTests(unittest.TestCase):
    def test_keyboard_runs_after_error_contract_behind_the_contention_guard(self):
        run_source = (E2E_ROOT / "run.sh").read_text()
        error_contract = run_source.index('source "$here/scenarios/error_contract.sh"')
        source_line = 'source "$here/scenarios/keyboard.sh"'
        registered = run_source.index(source_line)
        guard = run_source.rindex(CONTENTION_GUARD, 0, registered)
        self.assertLess(error_contract, guard)
        between = run_source[guard + len(CONTENTION_GUARD) : registered].strip().splitlines()
        self.assertEqual(between, ["# shellcheck source=tests/e2e/scenarios/keyboard.sh"])
        self.assertLess(registered, run_source.index('source "$here/scenarios/trace_performance.sh"'))
        self.assertLess(registered, run_source.index("cursor-multi-agent-probe.py"))

    def test_ref_churn_runs_after_the_probe_on_its_own_fixture_process(self):
        run_source = (E2E_ROOT / "run.sh").read_text()
        probe = run_source.index("cursor-multi-agent-probe.py")
        source_line = 'source "$here/scenarios/ref_churn.sh"'
        registered = run_source.index(source_line)
        self.assertLess(probe, registered)
        self.assertLess(registered, run_source.index("\nfinish", registered))
        preceding = run_source[:registered].rstrip().splitlines()[-2:]
        self.assertEqual(preceding[-1], "# shellcheck source=tests/e2e/scenarios/ref_churn.sh")
        self.assertNotIn(CONTENTION_GUARD, "\n".join(preceding))

    def test_ref_churn_reads_the_fixture_readouts_not_the_command_result(self):
        source = REF_CHURN.read_text()
        self.assertIn('wait_target "$stable" value 3000 --value stable-ok', source)
        self.assertIn("data.found", source)

    def test_interaction_scroll_asserts_without_a_defect_skip(self):
        source = INTERACTION.read_text()
        self.assertNotIn("skipmsg", source)
        self.assertIn('assert "[$mode] scroll moves content"', source)
        self.assertIn('"$scroll_before" != "$scroll_after"', source)

    def test_unknown_app_skip_is_gated_on_the_duplicate_pid_inventory_signature(self):
        source = ERROR_CONTRACT.read_text()
        self.assertIn('[ "$(json_field "$1" "$2.code")" = "TIMEOUT" ]', source)
        self.assertIn('= "global_window_inventory_unstable" ]', source)
        self.assertIn('"$2.details.skipped.0.field")" = "duplicate_pid" ]', source)
        self.assertIn("crates/macos/src/system/appkit_bridge.m", source)
        self.assertIn(
            'assert_error_envelope "unknown app is APP_NOT_FOUND" APP_NOT_FOUND 1 not_delivered',
            source,
        )
        self.assertIn('inventory_duplicate_pid_refusal "$RUN_JSON" data.results.1.error', source)

    def test_removed_element_leg_accepts_only_stale_ref_not_delivered(self):
        source = RELIABILITY.read_text()
        self.assertNotIn("STALE_REF|TIMEOUT", source)
        self.assertIn('"$stale_code" = "STALE_REF"', source)
        self.assertIn('"$stale_delivery" = "not_delivered"', source)
        self.assertIn('"$row_gone" = "1"', source)

    def test_new_legs_use_error_codes_that_exist_in_the_core_enum(self):
        known = core_error_codes()
        for path in (KEYBOARD, REF_CHURN, RELIABILITY):
            used = literal_error_codes(path.read_text())
            self.assertEqual(used - known, set(), path.name)
        self.assertIn("STALE_REF", literal_error_codes(RELIABILITY.read_text()))
        self.assertEqual(
            literal_error_codes(KEYBOARD.read_text()), {"POLICY_DENIED", "INVALID_ARGS"}
        )

    def test_press_scenario_observes_the_field_not_the_command(self):
        source = KEYBOARD.read_text()
        self.assertIn("await_value text-echo", source)
        self.assertIn('"$observed" = "$expected"', source)
        self.assertIn("press cmd+a", source)
        self.assertIn('press_expect "press cmd+a then delete empties the field" delete ""', source)
        self.assertIn("POLICY_DENIED 1 not_delivered", source)
        self.assertIn("INVALID_ARGS 1 not_delivered", source)


PRIVATE_ENV = "AGENT_DESKTOP_E2E_SKIP_PRIVATE"
PRIVATE_SITE_PATTERN = re.compile(
    r"osascript|System Events|list-notifications|NotificationCenter|dismiss-notification",
    re.IGNORECASE,
)
PRIVATE_GUARDS = {
    "scenarios/notifications.sh": (
        "if private_skip_enabled; then",
        'if osascript -e "display notification',
    ),
    "scenarios/os_surfaces.sh": (
        "if private_skip_enabled; then",
        'volume_original="$(os_volume)"',
    ),
    "scenarios/acceptance.sh": (
        "elif private_skip_enabled && [ \"$automation_state\" = \"denied\" ]; then",
        'nc_probe="$("$bin" list-notifications',
    ),
    "native-menus.py": (
        'os.environ.get("AGENT_DESKTOP_E2E_SKIP_PRIVATE") == "1"',
        "return run(binary)",
    ),
}


class PrivateSkipTests(unittest.TestCase):
    def run_bash(self, body: str, private: bool, calls: Path):
        mock_bin = calls.parent / "agent-desktop"
        mock_bin.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$MOCK_CALLS"\nexit 1\n')
        mock_bin.chmod(0o700)
        environment = os.environ.copy()
        environment.pop(PRIVATE_ENV, None)
        if private:
            environment[PRIVATE_ENV] = "1"
        environment["MOCK_CALLS"] = str(calls)
        script = (
            'source "$1"\n'
            'bin="$2"\n'
            f'json_tool="{E2E_ROOT / "json_tool.py"}"\n'
            'osascript() { echo "osascript $*" >> "$MOCK_CALLS"; return 1; }\n'
            "sleep() { :; }\n" + body
        )
        return subprocess.run(
            ["bash", "-c", script, "private", str(E2E_ROOT / "lib.sh"), str(mock_bin)],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )

    def test_every_private_site_in_the_suite_is_covered_by_a_guard(self):
        found = set()
        for path in sorted(E2E_ROOT.rglob("*")):
            relative = path.relative_to(E2E_ROOT).as_posix()
            if path.suffix not in (".sh", ".py") or relative.startswith("test_"):
                continue
            if PRIVATE_SITE_PATTERN.search(path.read_text()):
                found.add(relative)
        found.discard("lib.sh")
        self.assertEqual(found - set(PRIVATE_GUARDS), set(), "unguarded private site")

    def test_each_private_site_is_guarded_before_its_first_call(self):
        for relative, (guard, first_call) in PRIVATE_GUARDS.items():
            source = (E2E_ROOT / relative).read_text()
            self.assertIn(guard, source, relative)
            self.assertIn(first_call, source, relative)
            self.assertLess(source.index(guard), source.index(first_call), relative)

    def test_skip_helper_never_touches_the_pass_counter(self):
        body = r"""
pass=0; fail=0; skip=0; skips=()
private_skip "step one"
private_skip "step two"
printf 'pass=%s fail=%s skip=%s private=%s\n' "$pass" "$fail" "$skip" "${#private_skips[@]}"
"""
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_bash(body, True, Path(directory) / "calls")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("SKIP (private): step one", result.stdout)
        self.assertIn("SKIP (private): step two", result.stdout)
        self.assertNotIn("PASS", result.stdout)
        self.assertIn("pass=0 fail=0 skip=0 private=2", result.stdout)

    def test_summary_lists_the_private_skips_with_a_count(self):
        body = 'fixture_started=1\nprivate_skip "step one"\nprivate_skip "step two"\nfinish\n'
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_bash(body, True, Path(directory) / "calls")
        self.assertEqual(result.returncode, 0, result.stderr)
        summary = result.stdout.split("Summary", 1)[1]
        self.assertIn("passed: 0  failed: 0  skipped: 0", summary)
        self.assertIn("private skips: 2", summary)
        self.assertIn("- SKIP (private): step one", summary)
        self.assertIn("- SKIP (private): step two", summary)
        self.assertNotIn("all asserted E2E scenarios passed\n", summary)

    def test_summary_is_unchanged_when_the_variable_is_unset(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_bash("okmsg one >/dev/null\nfinish\n", False, Path(directory) / "calls")
        self.assertEqual(result.returncode, 0, result.stderr)
        summary = result.stdout.split("Summary", 1)[1]
        self.assertNotIn("private", summary)
        self.assertTrue(summary.endswith("  all asserted E2E scenarios passed\n"), summary)

    def test_notification_scenario_runs_nothing_private_when_skipped(self):
        scenario = E2E_ROOT / "scenarios" / "notifications.sh"
        body = f'source "{scenario}"\nfinish\n'
        with tempfile.TemporaryDirectory() as directory:
            calls = Path(directory) / "calls"
            result = self.run_bash(body, True, calls)
            self.assertFalse(calls.exists(), calls.read_text() if calls.exists() else "")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("SKIP (private): NT1-NT4", result.stdout)
        self.assertNotIn("PASS", result.stdout)
        self.assertNotIn("FAIL", result.stdout)

    def test_os_surfaces_scenario_never_calls_osascript_when_skipped(self):
        scenario = E2E_ROOT / "scenarios" / "os_surfaces.sh"
        body = f'source "{scenario}"\nfinish\n'
        with tempfile.TemporaryDirectory() as directory:
            calls = Path(directory) / "calls"
            result = self.run_bash(body, True, calls)
            recorded = calls.read_text() if calls.exists() else ""
        self.assertNotIn("osascript", recorded)
        self.assertIn("SKIP (private): OS surfaces Control Center volume", result.stdout)
        self.assertNotIn("PASS SKIP", result.stdout)

    def test_native_menus_skips_before_touching_the_machine(self):
        script = E2E_ROOT / "native-menus.py"
        environment = os.environ.copy()
        environment[PRIVATE_ENV] = "1"
        skipped = subprocess.run(
            ["python3", str(script), "--bin", "/nonexistent/agent-desktop"],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )
        self.assertEqual(skipped.returncode, 0, skipped.stderr)
        self.assertIn("SKIP (private): native TextEdit menu regression", skipped.stdout)
        self.assertIn("private skips: 1", skipped.stdout)
        self.assertNotIn("PASS", skipped.stdout)
        environment.pop(PRIVATE_ENV)
        unset = subprocess.run(
            ["python3", str(script), "--bin", "/nonexistent/agent-desktop"],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )
        self.assertNotEqual(unset.returncode, 0)
        self.assertNotIn("SKIP (private)", unset.stdout)


class FixtureLifecycleTests(unittest.TestCase):
    def run_lib(self, body: str) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as directory:
            mock_bin = Path(directory) / "agent-desktop"
            mock_bin.write_text("#!/bin/sh\nexit 1\n")
            mock_bin.chmod(0o700)
            script = (
                'source "$1"\n'
                'bin="$2"\n'
                "sleep() { :; }\n"
                'guard_exec() { current_pid="$next_pid"; return 0; }\n'
                "find_target() { return 0; }\n"
                "fixture_frame() { :; }\n"
                'pgrep() { [ -n "$current_pid" ] && printf "%s\\n" "$current_pid"; }\n' + body
            )
            return subprocess.run(
                ["bash", "-c", script, "lifecycle", str(E2E_ROOT / "lib.sh"), str(mock_bin)],
                capture_output=True,
                text=True,
                check=False,
            )

    def test_relaunch_moves_the_expected_owner_and_contention_still_catches_a_change(self):
        result = self.run_lib(
            "current_pid=; next_pid=111\n"
            "launch_fixture && check_fixture_contention && echo first-owner-ok\n"
            "current_pid=\n"
            "check_fixture_contention || echo vanished-caught\n"
            "contaminated=0\n"
            "next_pid=222\n"
            "launch_fixture && check_fixture_contention && echo relaunched-owner-ok\n"
            "current_pid=333\n"
            "check_fixture_contention || echo replaced-caught\n"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        for marker in ("first-owner-ok", "vanished-caught", "relaunched-owner-ok", "replaced-caught"):
            self.assertIn(marker, result.stdout)
        self.assertIn("expected pid '222', found '333'", result.stdout)

    def test_relaunch_refuses_a_fixture_it_did_not_launch(self):
        result = self.run_lib(
            "fixture_pid=111; fixture_started=1; current_pid=999; next_pid=999\n"
            "launch_fixture; status=$?\n"
            'printf "status=%s contaminated=%s owned=%s\\n" "$status" "$contaminated" "$fixture_owned"\n'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("status=1 contaminated=1 owned=0", result.stdout)
        self.assertIn("is already running as pid(s) 999", result.stdout)

    def test_relaunch_refuses_the_previous_pid(self):
        result = self.run_lib(
            "fixture_pid=222; fixture_started=1; current_pid=; next_pid=222\n"
            "launch_fixture; status=$?\n"
            'printf "status=%s contaminated=%s\\n" "$status" "$contaminated"\n'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("status=1 contaminated=1", result.stdout)
        self.assertIn("adopted the previous process pid 222", result.stdout)

    def test_force_close_observation_is_followed_by_a_relaunch_and_a_contention_check(self):
        scenario = (E2E_ROOT / "scenarios" / "trace_performance.sh").read_text()
        force_close = scenario.index('"$bin" close-app "$app" --force')
        relaunch = scenario.index("launch_fixture || abort_suite", force_close)
        self.assertLess(force_close, relaunch)
        run_source = (E2E_ROOT / "run.sh").read_text()
        trace = run_source.index('source "$here/scenarios/trace_performance.sh"')
        notifications = run_source.index('source "$here/scenarios/notifications.sh"')
        self.assertIn(CONTENTION_GUARD, run_source[trace:notifications])

    def test_launch_passes_the_normalized_frame_and_verifies_it_by_observation(self):
        launch = shell_function_body(E2E_ROOT / "lib.sh", "launch_fixture")
        self.assertIn('open_args+=(--env "AGENT_DESKTOP_FIXTURE_FRAME=$frame")', launch)
        self.assertIn('await_fixture_frame "$frame"', launch)
        self.assertIn('export AGENT_DESKTOP_FIXTURE_FRAME="$frame"', launch)
        safe_semantic = (E2E_ROOT / "safe-semantic.sh").read_text()
        self.assertIn(
            'AGENT_DESKTOP_FIXTURE_FRAME="$fixture_frame" AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 "$fixture_executable"',
            safe_semantic,
        )
        swift = (E2E_ROOT.parent / "fixture-app" / "FixtureLaunchFrame.swift").read_text()
        self.assertIn('"AGENT_DESKTOP_FIXTURE_FRAME"', swift)

    def test_frame_check_accepts_a_visible_area_cut_and_rejects_a_moved_or_narrow_window(self):
        result = self.run_lib(
            'frame_fits_request 20,64,1000,868 21,63,1001,868 && echo exact\n'
            'frame_fits_request 20,64,1000,868 20,64,1000,795 && echo dock-cut\n'
            'frame_fits_request 20,64,1000,868 20,34,1000,825 || echo above-request\n'
            'frame_fits_request 20,64,1000,868 20,64,900,868 || echo narrower\n'
            'frame_fits_request 20,64,1000,868 20,64,1000,600 || echo too-short\n'
            'frame_fits_request 20,64,1000,868 20,64 || echo malformed\n'
        )
        self.assertEqual(
            result.stdout.split(),
            ["exact", "dock-cut", "above-request", "narrower", "too-short", "malformed"],
            result.stderr,
        )


MANUAL_PRIVATE_SCRIPTS = {"native-menus.py"}


class DisplayPreconditionTests(unittest.TestCase):
    def check(self, width: int, height: int) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as directory:
            display = Path(directory) / "agent-desktop"
            display.write_text(
                "#!/bin/sh\n"
                f"echo '{{\"ok\":true,\"data\":[{{\"bounds\":{{\"x\":0,\"y\":0,\"width\":{width},\"height\":{height}}}}}]}}'\n"
            )
            display.chmod(0o700)
            return subprocess.run(
                ["bash", "-c", 'source "$1"; require_display_fits_fixture "$2" && echo fits', "check",
                 str(E2E_ROOT / "harness.sh"), str(display)],
                capture_output=True,
                text=True,
                check=False,
            )

    def test_a_display_that_leaves_room_for_the_minimum_frame_passes(self):
        result = self.check(1470, 956)
        self.assertEqual(result.stdout.strip(), "fits", result.stderr)

    def test_a_small_display_stops_with_one_blocked_message(self):
        result = self.check(1024, 768)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SKIP (blocked): the largest display leaves a fixture frame of 20,64,984,680", result.stderr)
        self.assertIn("at least 980x748 points", result.stderr)

    def test_run_checks_the_display_before_any_scenario(self):
        source = (E2E_ROOT / "run.sh").read_text()
        gate = source.index('require_display_fits_fixture "$release_bin" || exit 2')
        self.assertLess(gate, source.index('bash "$here/safe-semantic.sh"'))


class LaunchFrameParseTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "darwin" and shutil.which("swiftc"), "needs swiftc on macOS")
    def test_frame_parsing_is_all_or_nothing(self):
        cases = {
            "20,64,1000,868": "20.0,64.0,1000.0,868.0",
            " 1, 2, 3, 4": "1.0,2.0,3.0,4.0",
            "10,x,800,900,50": "default",
            "10,20,800,900,50": "default",
            "1,2,3": "default",
            "1,,3,4": "default",
            "nan,0,800,900": "default",
            "0,0,inf,900": "default",
            "0,0,0,900": "default",
            "0,0,800,-1": "default",
        }
        with tempfile.TemporaryDirectory() as directory:
            main = Path(directory) / "main.swift"
            main.write_text(
                "for raw in CommandLine.arguments.dropFirst() {\n"
                "    if let f = FixtureLaunchFrame.parse(raw) {\n"
                '        print("\\(f.origin.x),\\(f.origin.y),\\(f.width),\\(f.height)")\n'
                '    } else { print("default") }\n'
                "}\n"
            )
            binary = Path(directory) / "parse"
            subprocess.run(
                ["swiftc", "-o", str(binary), str(main),
                 str(E2E_ROOT.parent / "fixture-app" / "FixtureLaunchFrame.swift")],
                check=True,
                capture_output=True,
            )
            result = subprocess.run([str(binary), *cases], capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.splitlines(), list(cases.values()))


class HarnessSafetyTests(unittest.TestCase):
    def test_no_harness_file_uses_system_events_or_a_full_screen_capture(self):
        paths = [
            path
            for path in sorted(E2E_ROOT.rglob("*"))
            if path.suffix in (".sh", ".py")
            and not path.name.startswith("test_")
            and path.name not in MANUAL_PRIVATE_SCRIPTS
        ]
        paths.append(E2E_ROOT.parent.parent / "scripts" / "cursor-multi-agent-probe.py")
        for path in paths:
            source = path.read_text()
            self.assertNotIn("System Events", source, path.name)
            self.assertNotIn("screencapture", source, path.name)

    def test_run_never_calls_a_manual_private_script(self):
        run_source = (E2E_ROOT / "run.sh").read_text()
        for name in MANUAL_PRIVATE_SCRIPTS:
            self.assertNotIn(name, run_source)

    def test_safety_checkpoint_never_prints_the_window_list(self):
        source = shell_function_body(E2E_ROOT / "safe-semantic.sh", "safety_checkpoint")
        self.assertIn("focus-summary", source)
        self.assertNotRegex(source, r"printf [^\n]*\$windows")


class PrivateScenarioListTests(unittest.TestCase):
    def declared(self) -> list:
        result = subprocess.run(
            ["bash", "-c", 'source "$1"; printf "%s\\n" "${private_scenarios[@]}"', "list", str(E2E_ROOT / "lib.sh")],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout.splitlines()

    def test_every_private_skip_names_a_declared_scenario_and_every_scenario_is_used(self):
        used = []
        for path in sorted((E2E_ROOT / "scenarios").glob("*.sh")):
            for label in re.findall(r'private_skip "([^"]+)"', path.read_text()):
                used.append(label.split(" (", 1)[0])
        self.assertEqual(sorted(used), sorted(self.declared()))

    def test_list_skipped_prints_the_scenarios_and_builds_nothing(self):
        with tempfile.TemporaryDirectory() as directory:
            calls = Path(directory) / "calls"
            cargo = Path(directory) / "cargo"
            cargo.write_text(f'#!/bin/sh\necho cargo >> "{calls}"\n')
            cargo.chmod(0o700)
            environment = os.environ.copy()
            environment["PATH"] = f"{directory}{os.pathsep}{environment['PATH']}"
            result = subprocess.run(
                ["bash", str(E2E_ROOT / "run.sh"), "--list-skipped"],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
            self.assertFalse(calls.exists())
        self.assertEqual(result.returncode, 0, result.stderr)
        line = result.stdout.strip()
        self.assertTrue(line.startswith("AGENT_DESKTOP_E2E_SKIP_PRIVATE=1 skips 3 scenario(s): "), line)
        for scenario in self.declared():
            self.assertIn(scenario, line)


class PythonResolutionTests(unittest.TestCase):
    def resolve(self, names: dict, preset: str = "") -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as directory:
            for name, target in names.items():
                path = Path(directory) / name
                if target is None:
                    path.write_text("#!/bin/sh\necho 'Python 3.9.6'\nexit 1\n")
                    path.chmod(0o700)
                else:
                    path.symlink_to(target)
            environment = {"PATH": directory, "HOME": directory}
            if preset:
                environment["AGENT_DESKTOP_E2E_PYTHON"] = preset
            return subprocess.run(
                [
                    "/bin/bash",
                    "--norc",
                    "-c",
                    'source "$1"; require_e2e_python && printf "%s" "$e2e_python"',
                    "resolve",
                    str(E2E_ROOT / "python.sh"),
                ],
                capture_output=True,
                stdin=subprocess.DEVNULL,
                text=True,
                env=environment,
                check=False,
            ), directory

    def test_prefers_a_versioned_interpreter_over_an_old_python3(self):
        (result, directory) = self.resolve({"python3": None, "python3.12": sys.executable})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"{directory}/python3.12")

    def test_uses_python3_when_it_is_new_enough(self):
        (result, directory) = self.resolve({"python3": sys.executable})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"{directory}/python3")

    def test_fails_with_what_it_found_and_what_to_do(self):
        (result, _) = self.resolve({"python3": None})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertIn("needs Python 3.10 or newer", result.stderr)
        self.assertIn("(Python 3.9.6)", result.stderr)
        self.assertIn("AGENT_DESKTOP_E2E_PYTHON", result.stderr)

    def test_an_old_override_is_ignored(self):
        (result, directory) = self.resolve(
            {"python3": None, "python3.11": sys.executable}, preset="/usr/bin/false"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, f"{directory}/python3.11")
        self.assertIn(
            "AGENT_DESKTOP_E2E_PYTHON=/usr/bin/false is not Python 3.10 or newer; ignoring it",
            result.stderr,
        )


if __name__ == "__main__":
    unittest.main()
