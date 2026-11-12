# Hermes ネイティブ plugin hook API と ECC 対応

## 調査範囲と根拠

Hermes ソースは `~/.hermes/hermes-agent` の commit `3b421578588243f920886e6fe79a6faa87ea026e` を固定して調査した。
根拠となる主要ファイルは `hermes_cli/plugins.py`（イベント登録・dispatch）、`website/docs/user-guide/features/hooks.md`（公開契約）、`agent/shell_hooks.py`（shell 応答解析）である。
以下の行番号はこの commit における行番号である。
「fact」はソースまたは公式ドキュメントに明記された契約であり、「推測」はECCとの機能同等性の評価である。

## Native hook の登録・実行モデル

- **全イベント名**: `VALID_HOOKS` は次のイベントを許可する。`pre_tool_call`, `post_tool_call`, `transform_terminal_output`, `transform_tool_result`, `transform_llm_output`, `pre_llm_call`, `post_llm_call`, `on_stream_start`, `on_stream_delta`, `on_stream_end`, `on_interim_message`, `pre_verify`, `pre_api_request`, `post_api_request`, `api_request_error`, `pre_auxiliary_call`, `post_auxiliary_call`, `transform_api_error_classification`, `on_session_start`, `on_session_end`, `on_session_finalize`, `on_session_reset`, `on_session_delete`, `on_session_open`, `on_skill_lifecycle`, `subagent_start`, `subagent_stop`, `pre_gateway_dispatch`, `post_gateway_admission`, `agent_loop_stopped`, `pre_approval_request`, `post_approval_response`, `on_human_input_request`, `on_human_input_resolved`, `on_room_member_activity`, `pre_transcription`, `kanban_task_claimed`, `kanban_task_completed`, `kanban_task_blocked`, `on_kanban_worker_spawned`, `on_kanban_worker_exited`, `on_kanban_worker_stale_claim`, `on_kanban_task_updated`, `on_kanban_dispatch_tick`, `gateway_platform_event`, `pre_command`, `post_compaction`, `post_curator_run` (計48)。根拠: `hermes_cli/plugins.py:111-210`。
- Python callbackにはキーワード引数が渡され、公式文書は “Always accept `**kwargs` for forward compatibility.” と説明する。例外は記録して読み飛ばし、後続callbackを続行する (`hooks.md:394-395`)。
- timeout対象Python callback (hot-path observerと `pre_tool_call`) の既定timeoutは30秒、上限600秒で、0は無効化を表す。`pre_tool_call` のtimeoutはfail-closed、それ以外のbounded hookはfail-openとなる (`hooks.md:396`)。`subagent_stop` はcaller threadで実行する。shell callbackはエントリごとのtimeoutを使い、既定60秒、上限300秒となる (`shell_hooks.py:37-38`; `hooks.md:1980-1982`)。
- 公開比較は「Plugin hooks」と「Shell hooks」の双方をCLI/Gatewayで利用可能とし、双方のblock対応は `pre_tool_call`、context注入対応は `pre_llm_call` とする (`hooks.md:1961-1970`)。例外はPython plugin専用の `transform_api_error_classification` である (`hooks.md:473`; `plugins.py:213-215`)。shellのblock解析は `pre_tool_call` のexit-2/fail-closedのみ対応し、専用応答parserは `pre_tool_call`/`pre_llm_call`/`pre_verify` に限られる (`shell_hooks.py:41-45,642-684`)。
- streaming observer群と `on_room_member_activity` はtoken path外の有界queueで非同期配信する。遅延したobserverは自身の最古のeventを失う場合がある (`hooks.md:469-472,494,508-509`)。その他は通常の `invoke_hook` 経路を使い、timeout対象callbackはworkerで実行する場合がある。`subagent_stop` はcaller threadで実行する (`hooks.md:396`)。
- 全callback入力はkwargsで、相関ID等の一部fieldはeventによって欠落する (`hooks.md:394,398`)。イベント固有field、timing、return contractは次の一覧に示す。

## イベント契約一覧

以下で「shell + plugin」は両方式で登録できること、「plugin only」はshell設定から拒否されることを表す。「queued async」はtoken path外のqueue配送であり、それ以外の通常dispatchは同期callbackとして扱う。ただしtimeout対象はbounded workerへ移す場合がある。全イベント48件を `plugins.py:111-210` で確認した。

| イベント | kwargs / 発火タイミング | 戻り値・制御 | 実行/timeout・例外 |
|---|---|---|---|
| `pre_tool_call` | `tool_name,args,task_id,session_id,conversation_id,tool_call_id,turn_id,api_request_id,middleware_trace`。tool 実行直前 | block/approve/modify。valid block が approve より優先し、modify は args に shallow merge | bounded。timeout・例外は fail-closed (tool block)。|
| `post_tool_call` | 同上に `result,duration_ms,status,error_type,error_message`。成功/失敗/blocked の後 | observer、無視 | bounded、その他 timeout/exception は fail-open |
| `transform_tool_result` | tool result/args/IDs。`post_tool_call` 後、conversation append 前 | 最初の string が result を置換 | transform。|
| `transform_terminal_output` | `command,output,returncode,task_id,env_type`。foreground process capture 後 | 最初の string が output を置換 | transform。|
| `pre_transcription` | `file_path,provider,model,language,prompt,source`。provider 解決後、backend 前 | prompt/language/model を順序適用、file_path は read-only | transform。|
| `pre_llm_call` | `session_id,task_id,turn_id,user_message,conversation_history,is_first_turn,model,platform,parent_session_id,sender_id`。turn開始前 | string/context を user message に結合。block なら推論前にturn終了 | bounded hook、通常fail-open。|
| `post_llm_call` | `session_id,task_id,turn_id,user_message,assistant_response,conversation_history,model,platform,side_question_token`。成功・非中断 turn の finalization | observer | callback隔離。|
| `transform_llm_output` | `response_text,session_id,model,platform`。最終配信前、post_llm_callより前 | 最初のnon-empty stringが応答置換 | transform。|
| `pre_verify` | `session_id,platform,model,coding,attempt,final_response,changed_paths`。編集後のverify gate | continue/block-stopで継続。nudge回数上限あり | `agent.max_verify_nudges`でbounded。|
| `pre_api_request` | provider attemptごと。request/turn/session/provider/model/retry等 (`hooks.md:464`) | observer、戻り値無視 | fail-open。|
| `post_api_request` | 正規化されたprovider成功後。response/usage/finish等 (`hooks.md:465`) | observer | fail-open。|
| `api_request_error` | failed provider attemptごと。error/retry等 (`hooks.md:466`) | observer | fail-open。|
| `pre_auxiliary_call` / `post_auxiliary_call` | auxiliary LLM physical attemptごと。API requestと同形 + `aux_task` (`hooks.md:467-468`) | observer、戻り値無視 | fail-open。|
| `transform_api_error_classification` | failed request前、provider/model/status/error/token/context等 (`plugins.py:127-133`) | `reason`必須の分類dict。全callback実行し最初の有効dict | Python plugin専用。|
| `on_stream_start` / `on_stream_delta` / `on_stream_end` / `on_interim_message` | stream start/delta/endまたは中間assistant message。turn/model/session等 (`hooks.md:469-472`) | immutable observer、戻り値無視 | token path外のqueue。遅いdelta subscriberは自身の古いeventをdrop。|
| `on_session_start` | `session_id,model,platform`。新sessionの最初のturn | observer、戻り値無視 | 通常callback隔離。|
| `on_session_open` | `session_id,platform,reason`。ローカルsession create/open時、最初のturn前 | observer、戻り値無視 | timeout bounded。メッセージングchatでは不発。|
| `on_session_end` | canonical payload `session_id,task_id,turn_id,completed,failed,interrupted,turn_exit_reason,model,platform`。turn finalization | observer | 戻り値無視。exit pathに縮小legacy payloadあり。|
| `on_session_finalize` / `on_session_reset` / `on_session_delete` | session lifecycle IDs/platform/reason等 (`hooks.md:477-479`) | observer | 戻り値無視。|
| `post_compaction` | `session_id,old_session_id,kind,reason,before_tokens,after_tokens,messages_removed`。commit済compaction後のみ | observer、戻り値無視 | timeout-bounded、fail-open。|
| `on_skill_lifecycle` | `action,skill_name,provenance,task_id,session_id,use_count,reused,reuse_after_patch`。authoritative skill state change後 | observer | 戻り値無視。|
| `subagent_start` / `subagent_stop` | parent/child session, role, goal/summary/status/tool history等 (`hooks.md:484-485`)。child開始/終了 | observer | `subagent_stop` caller-thread契約はtimeout workerへ移さない (`hooks.md:396`)。|
| `pre_gateway_dispatch` | `event,gateway,session_store`。incoming non-internal MessageEvent ingress | skip/rewrite/allow。skipはdrop、rewriteはevent.text差替え | directive hook。|
| `post_gateway_admission` | gateway admission後。詳細はhooks.mdの該当節 | observer/fail-open | fail-open。|
| `agent_loop_stopped` | `session_key,platform,reason,invalidation_reason`。実行中agent interrupt後 | 戻り値無視 | observer。|
| `pre_approval_request` / `post_approval_response` | command/description/pattern/session/surface。post側はchoice/decided_by追加 | observer。vetoは `pre_tool_call` | 戻り値無視。|
| `on_human_input_request` / `on_human_input_resolved` | human-input lifecycle情報 | observer | 戻り値無視。|
| `on_room_member_activity` | room/thread/member/turn/task/execution_generation/kind/seq/payload | client-safe observer。room durable logには書かない | queue経由。|
| `kanban_task_claimed/completed/blocked` | task_id,board,assignee,run_id,profile_name。completedはsummary、blockedはreason追加 | observer、DB commit後 | slow callbackがSQLite writer lockを保持しない。|
| `on_kanban_worker_spawned/exited/stale_claim`, `on_kanban_task_updated`, `on_kanban_dispatch_tick` | task/profile/run共通情報 + event固有worker/status/changed_fields/result | observer、戻り値無視 | dispatch tickはlock解放後。|
| `gateway_platform_event` | `platform,event_type,payload`。Telegram reaction/message_edited、Discord message_edited/deleted/thread_created/thread_renamed | observer | payloadはevent固有。|
| `pre_command` | `surface,command,alias_used,args_raw,session_key,platform`。recognised slash command handler前 | observer v1、戻り値無視 | bounded per callback; `pre_command` dispatch is serialized per session (`hooks.md:489`) |
| `post_curator_run` | `run_dir,run_json_path,report_md_path,dry_run`。curator report書込後 | observer | 戻り値無視 (`hooks.md:481`) |

**登録可能チャネル・同期性**。公開表は「Events: `VALID_HOOKS`」とし、Shell hooks/Plugin hooks双方をCLI + Gatewayでサポートする (`hooks.md:1961-1967`)。従って48イベントは原則shell + pluginである。例外は `transform_api_error_classification` で、公式表に “Python plugins only” とある (`hooks.md:473`)。shellを含む全コールバックはkwargsで呼ばれ、shell wireは標準入力のJSON、応答は標準出力JSON (`shell_hooks.py:1-5`; `hooks.md:1991-2011`)。stream observers (`on_stream_start/delta/end`, `on_interim_message`) と `on_room_member_activity` は明記されたqueued/off-token-path deliveryである (`hooks.md:469-472,494,508-509`)。その他は通常の `invoke_hook` 経路で、Pythonのtimeout-bounded callbackは既定30秒/max600秒 (`hooks.md:396`)、shellは既定60秒/max300秒 (`shell_hooks.py:37-38`; `hooks.md:1980-1982`)。戻り値でagent制御可能なものは `pre_tool_call`, `pre_llm_call`, `pre_verify`, `pre_gateway_dispatch`, `post_gateway_admission` とtransform系に限られ、その他observerの戻り値は無視される (`hooks.md:455-489`)。

引用例: “Callbacks receive **keyword arguments**.” (`hooks.md:394`); “other bounded hooks fail open (skip)” (`hooks.md:396`); “return ignored” (`hooks.md:456`); “Python plugins only” (`hooks.md:473`)。ここでいう同期はcallbackがeventの通常dispatchで実行されることを示す。bounded hookではtimeout workerに隔離される場合があり、caller-thread保証があるものは別記する。

根拠: イベント全名・追加契約は `plugins.py:111-210`; kwargs、具体的タイミングと戻り値は `hooks.md:453-486`。Shell parser の制御チャネルは `shell_hooks.py:637-684` にあり、blockは原則pre_tool_call、pre_llm_call、pre_verifyに限定される。

## Shell hook と plugin-only の区別

Shell hooks は `VALID_HOOKS` を allow-list として利用し、通常のhookはJSON stdoutを解釈する。`pre_tool_call` はblock/approve/modify、`pre_llm_call`はblock/context、`pre_verify`はcontinueを扱う。shell 側のblocking event setは `pre_tool_call` (`shell_hooks.py:44-45`) であり、shell response parserは個別dialectを `_parse_pre_tool_call`, `_parse_pre_verify`, `_parse_pre_llm_call` に分ける (`shell_hooks.py:642-684`)。それ以外はobserver/transform用戻り値であり、Python pluginとshell双方の登録可能性はイベントごとに異なる。少なくとも `transform_api_error_classification` はPython plugin専用 (`plugins.py:213-215`)。

## ECC 8イベントのHermes側候補

- **PreToolUse → `pre_tool_call` (fact: 部分代替)**。tool実行直前にkwargsがあり、blockおよびargs rewrite (`modify`) が可能 (`hooks.md:455`)。ECCのtool名・入力条件に相当する。**同等でない点**: ECC固有のregistration/子dispatcher、追加context提示の一部、ECCの個別policyやstateは自動移植されない。Hermesの戻り値契約とタイミングが重なるだけで、個別hookごとのparityは推測に留まる。
- **UserPromptSubmit → `pre_llm_call` (fact: 近い代替)**。毎turnの推論前に発火し、contextをuser messageに注入、turnをblockできる。`is_first_turn`も渡る (`hooks.md:460`)。**同等でない点**: gateway ingress前、auth/pairing前の入力書換えとは異なる。ECCのHookify prompt-rule dialectや同一CLI prompt hookの実行保証は推測であり、Hermes event semanticsと同一ではない。
- **PreCompact → `post_compaction` (fact: 境界は存在するがpre代替なし)**。compaction commit後にobserver通知する (`plugins.py:207`; `hooks.md:480`)。**同等でない点**: compaction前に状態を保存できない。pre hookは調査した `VALID_HOOKS` にない。`pre_llm_call`等で次turn contextを復元する案はpost-commit通知後の保存・次turn注入であり、直前状態の保存と同等ではない。
- **SessionStart → `on_session_open` または `on_session_start` (fact: lifecycle observer)**。`on_session_open`はlocal client create/open時、最初のturn前。`on_session_start`は新sessionの最初のturnに発火し、両者の戻り値は無視される (`hooks.md:474-475`)。**復元context**: `pre_llm_call`で `is_first_turn=true` を見て注入する方法は可能な設計案 (推測)。`on_session_start`から値を返してpromptに直接注入するのは契約上できない (fact)。session open と first turn の間に作業する要件なら `on_session_open` で外部stateを準備し `pre_llm_call`で読む案があるが、race/再開/各client経路は別途検証が必要。
- **PostToolUse → `post_tool_call` (fact: 範囲の近い代替)**。成功・失敗・blocked後にstatus/result/error込みで発火し戻り値無視 (`hooks.md:456`)。**同等でない点**: ECC dispatcherの子hook制御、同期/非同期分離、stderr/contextの配信保証は同じでない。
- **PostToolUseFailure → `post_tool_call` の `status`/`error_type`/`error_message` (fact: failure情報を含む)**。Hermesは失敗専用イベントでなく成功・失敗共通eventである (`hooks.md:456`)。条件分岐で失敗を選別できる。**同等でない点**: failed-only fire-siteとECC MCP health/reconnect stateの提供・復旧挙動は存在しない。`status`の全値と各toolでの失敗表現は未検証。
- **Stop → `post_llm_call` または `agent_loop_stopped` / `pre_verify` (用途別; 推測を含む)**。成功した非中断turn finalizationには`post_llm_call`が近く (`hooks.md:461`)、実行中turnの`/stop`等interruptには`agent_loop_stopped`が近い (`plugins.py:144-147`; `hooks.md:482`)。編集後の回答受理前に続行させるものは`pre_verify` (`hooks.md:463`)。**同等でない点**: 単一のECC Stop境界とは違い用途でイベントが分かれる。`post_llm_call`の戻り値は無視されるため、終了阻止・再実行指示はできず、`agent_loop_stopped`も戻り値無視。`pre_verify`は編集したturnのbounded gateに限られ、一般的Stop hookでない。
- **SessionEnd → `on_session_end` または `on_session_finalize` (fact: 近いlifecycle観測)**。`on_session_end`はcanonicalでは各turn finalizationであり、CLI/TUI exitにはreduced legacy shapeがある。`on_session_finalize`はCLI/TUI/gateway teardown (`hooks.md:476-477`)。**同等でない点**: ECC SessionEndのreason/source/cleanup範囲との一対一対応は未確定。event戻り値は無視され、会話への注入・turn制御はできない。

## 「再現不能」の判定基準案

再現不能と記載する基準は、単に同名イベントがないことではなく、対象動作に必要な境界・入力・制御結果のすべてを代替イベントのkwargsと返却契約で実現できず、後段イベント等の組合せでも安全に同じ観測・制御を作れないこととする。

`INVENTORY.md` の No equivalent 20件から、基準に該当しうるものは次の通りである (現時点の一次仕分けであり、個別実装を新たに実行確認した意味ではない)。

- **pre-write doc warning**: `pre_tool_call`にwarningをモデルへ追加する専用context戻り値がなく、stderrもモデルへの注入契約ではない。追加context要求が要件なら該当する。
- **manual compaction suggestion**: `post_compaction`は事後observer。提案を会話へ届ける必要がある場合、境界・配信channelがなく該当する。
- **MCP health preflight / post-failure reconnect**: health state/reconnect APIをhook kwargs/戻り値が提供せず、`post_tool_call`もobserverのみ。adapterが独自にMCP状態を構築しない限り該当する。
- **file-aware GateGuard**: `pre_tool_call`はargsを渡すが、ECCのper-file/per-turn state、hard-link判定、target semanticsや質問フローの同等性は資料上確認できない。これらの特性が必須なら該当する。
- **PreCompact state capture**: 事前hookがなく、post hookでは元contextの消失前保存を再現できず該当する。
- **SessionStart restored context**: `on_session_start`戻り値は無視されるため、そのcallback自体での注入は不可。`pre_llm_call`のfirst-turn注入は別機構による回避案として評価する。
- **Plan Canvas SessionStart/Stop feedback queue**: 該当する専用queue/APIが見当たらず、対応するgateway/CLI state連携も必要。該当。
- **PostToolUse async child dispatcher/control preservation**: Hermes shell hookの個別entryがECC子hookのbackground scheduling/controlを提供する契約ではない。ECC同等の子別制御が必須なら該当。
- **Stop hookify / format-typecheck / session persistence / session evaluation / cost tracker / desktop notification**: `post_llm_call`や`on_session_end`はobserverであり終了を止められない。format/typecheck等を実行する新しいintegrationや集計・notification adapterなしでは該当しうる。`pre_verify`は編集後限定の続行gateなのでStop全体の代替ではない。
- **Per-response changed-file console-log scan**: `post_tool_call`は利用可能だが、全response単位のchanged-file setやuser-visible warningが保証されない場合に限り該当する。イベントなしと断定せず、partialとするのが妥当。

なお `INVENTORY.md` の既存statusと上記分類はECC仕様とHermes契約の比較であり、Hermes実runtimeでのdeliveryを意味しない。

## 未検証点

- Hermes runtime/gatewayで各Python plugin・shell hookのdelivery順、kwargs実値、dispatcher behaviorを実行確認していない。
- `invoke_hook()`隔離環境からの直接実行確認はこの調査では行っていない。したがって一切「実行確認済み」とは記載しない。
- `status`/`error_type`等の実値、各toolでのfailure表現、例外/timeout後の複数callback間の継続はソース記載以上の挙動を確認していない。
- SessionStartで復元したcontextを`pre_llm_call`がfirst turnへ注入する案の、CLI/TUI/Desktop/gateway/resume各経路の順序・raceは未検証。
- ECC dispatcher/control parity、shell hookの実配信・shell unsupported以外の全イベント対応状況は実運用構成で未検証。
- 行番号は上記のHermes commitに結び付くが、将来のHermes変更で契約が変わる可能性がある。
