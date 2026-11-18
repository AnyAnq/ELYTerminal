# AI assistant

English / [中文](../zh_cn/ai.md)

Click Ely to open or hide the assistant inside the main window. A press still pets her; dragging moves her without opening the panel. Turn her on in Settings → Terminal → Ely if she is hidden.

1. Open Settings → AI models, or the assistant's gear button.
2. Enter a configuration name, service address and model identifier. The address may be a base such as `https://api.openai.com/v1`, or the complete `/chat/completions` endpoint. The service must support streamed OpenAI-compatible Chat Completions.
3. Enter an API key if the service requires one. Keys use the system credential store. The settings file holds model metadata alone. A blank key keeps the saved key for that address; Clear saved key removes it. A local service may be keyless.
4. Save, then Test connection. The test checks streamed chat, a function call and the reply to its result. A service without working tools is labeled Chat only. Repeat the test after changing the address, model or key.
5. Select the model in the assistant, preview the attached context, describe the problem and send.

Auto allow sits beside the model selector in the assistant and is off by default. Enabling it resumes a pending approval and allows later actions without repeated prompts, including deletion, installation and push. The choice is saved; turning it off restores approval for later commands. Hover over the switch for its explanation. Pane binding, Stop, manual takeover and the 30-call limit remain in force. Enable it for tasks you trust; completed changes are not undone.

“Chat only · test tool calling first” means this configuration has not passed the tool check, rather than proving the model lacks tools. “Chat only · unsupported terminal” identifies a pane that cannot execute commands. DeepSeek thinking uses automatic tool choice and returns the required `reasoning_content` in subsequent tool messages; failed tool tests show the error. After upgrading, retest configurations previously marked chat only.

Each new request attaches the pane's shell and project directory, visible terminal text, the latest 200 lines and the selection. Preview refreshes this context. Terminal content is sent to the selected service, so check it for sensitive text before sending.

On Windows, an integrated Windows PowerShell, pwsh or cmd pane can run commands. The assistant can inspect UTF-8 project files, make exact edits and verify the result through that pane. A handed or explicitly launched program, a custom shell command with arguments, Git Bash, WSL and non-Windows panes initially support chat and attached context alone.

The task stays bound to the pane from which it started. File tools use the directory where the task began, including when you resume it with Continue task. A new request uses the pane's current directory. Only one task runs across the app. Hiding the panel keeps it running. Closing its pane cancels the task and clears its conversation and drafts. New conversation clears the current pane's conversation. Conversations live in memory until you close the app.

Use the copy icon beside a message, note or tool to copy its full text. A tool copies its name, arguments and complete output, even when collapsed. Long replies copy every chunk; Markdown copies its source. Plain text, tool output and the context preview also support selection and Ctrl+C (Cmd+C on macOS); the preview has a full-copy button.

With the AI input focused, Alt+↑ recalls the previous submitted prompt and Alt+↓ moves forward; the arrow buttons beside the model do the same and focus the input. Edit the recalled text before sending. Moving past the latest prompt restores the unsent draft. Plain Up/Down still move the multiline caret. Prompt history belongs to the pane's current conversation and clears with New conversation or app exit; recalling never submits a request.

The assistant waits while a program runs. Interactive programs and password prompts require manual takeover. At a ready shell prompt, it clears unfinished input before submitting a command, including PowerShell continuation lines. That draft is not restored. Git commands, unknown or compound commands, destructive operations, elevation, global installations, pushes and deployment ask for approval by default. Automatic approval allows them directly. A refusal is returned to the model without executing the action.

Stop, manual typing, paste or Ctrl+C cancels further AI actions and attempts Ctrl+C on its current command. Input arriving during interruption waits for the shell's next prompt. If the program ignores Ctrl+C, interrupt it again or close the pane. Stopping does not undo completed commands or file edits. After 30 model calls, Continue task allows another 30.

The assistant uses the existing `curl` executable for model connections. A missing executable or failed connection is shown as an error.

Long replies use virtual text rows without losing content. Live output shows text; shorter completed replies show Markdown. Stream updates are batched and process checks run in the background to keep input and scrolling responsive.

The assistant gear opens the AI page on the first paint. Model selection does not reset unrelated terminal options or rescan find history; creating the separate settings window still incurs Windows platform work.
