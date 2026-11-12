import pytest
pytest = pytest.importorskip("pytest")
import shutil
from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook
if not hermes_runtime_available(): pytest.skip("Hermes source tree unavailable", allow_module_level=True)

def test_mcp_extension_is_registered_and_fails_open_on_non_mcp():
    with IsolatedHermesHome(enable_foundation=True):
        result=invoke_hook("pre_tool_call", tool_name="read_file", args={"path":"x"}, session_id="runtime-mcp")
        assert not any(isinstance(r,dict) and r.get("action")=="block" for r in result)

def test_gateguard_extension_blocks_first_edit_then_allows_retry():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        first=invoke_hook("pre_tool_call", tool_name="write_file", args={"path":"/tmp/runtime-gateguard-test"}, session_id="runtime-gateguard")
        assert any(isinstance(r,dict) and r.get("action")=="block" for r in first)
        second=invoke_hook("pre_tool_call", tool_name="write_file", args={"path":"/tmp/runtime-gateguard-test"}, session_id="runtime-gateguard")
        assert not any(isinstance(r,dict) and r.get("action")=="block" for r in second)
