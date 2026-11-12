import pytest
pytest = pytest.importorskip("pytest")
import shutil
from hermes_runtime import BINARY_PATH, IsolatedHermesHome, hermes_runtime_available, invoke_hook
if not hermes_runtime_available(): pytest.skip("Hermes source tree unavailable", allow_module_level=True)

def test_mcp_extension_is_registered_and_fails_open_on_non_mcp():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        result=invoke_hook("pre_tool_call", tool_name="read_file", args={"path":"x"}, session_id="runtime-mcp")
        assert not any(isinstance(r,dict) and r.get("action")=="block" for r in result)

def test_mcp_failure_and_preflight_flow_through_hermes_dispatcher():
    with IsolatedHermesHome(enable_foundation=True) as runtime:
        shutil.copy2(BINARY_PATH, runtime.bin / "foundation")
        invoke_hook("post_tool_call", tool_name="mcp__runtime__search", args={}, status="error", error_type="tool_error", session_id="runtime-mcp-flow")
        result=invoke_hook("pre_tool_call", tool_name="mcp__runtime__search", args={}, session_id="runtime-mcp-flow")
        assert any(isinstance(r,dict) and r.get("action")=="block" for r in result)
