// A trimmed copy of monaco-editor/esm/vs/editor/editor.main.js (monaco-editor 0.57): the editor core plus find,
// folding-free editing helpers, suggest and bracket matching, without the 80+ language definitions, the CSS/HTML/JSON/
// TypeScript services (9 MB of workers), the LSP client, the diff editor, color picker, code lens, inlay hints and the
// other contributions Tez does not use. Compare with editor.main.js after upgrading monaco-editor.

import 'monaco-editor/editor/contrib/anchorSelect/browser/anchorSelect.js';
import 'monaco-editor/editor/contrib/bracketMatching/browser/bracketMatching.js';
import 'monaco-editor/editor/contrib/caretOperations/browser/transpose.js';
import 'monaco-editor/editor/contrib/clipboard/browser/clipboard.js';
import 'monaco-editor/editor/contrib/codeAction/browser/codeActionContributions.js';
import 'monaco-editor/editor/browser/widget/codeEditor/codeEditorWidget.js';
import '../node_modules/monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon.css';
import 'monaco-editor/editor/contrib/comment/browser/comment.js';
import 'monaco-editor/editor/contrib/contextmenu/browser/contextmenu.js';
import 'monaco-editor/editor/contrib/cursorUndo/browser/cursorUndo.js';
import 'monaco-editor/features/find/register.js';
import 'monaco-editor/editor/contrib/fontZoom/browser/fontZoom.js';
import 'monaco-editor/editor/contrib/gotoError/browser/gotoError.js';
import 'monaco-editor/editor/contrib/indentation/browser/indentation.js';
import 'monaco-editor/editor/contrib/insertFinalNewLine/browser/insertFinalNewLine.js';
import 'monaco-editor/editor/contrib/linesOperations/browser/linesOperations.js';
import 'monaco-editor/editor/contrib/longLinesHelper/browser/longLinesHelper.js';
import 'monaco-editor/editor/contrib/middleScroll/browser/middleScroll.contribution.js';
import 'monaco-editor/editor/contrib/suggest/browser/suggestInlineCompletions.js';
import 'monaco-editor/editor/standalone/browser/toggleHighContrast/toggleHighContrast.js';
import 'monaco-editor/editor/contrib/unusualLineTerminators/browser/unusualLineTerminators.js';
import 'monaco-editor/editor/contrib/wordOperations/browser/wordOperations.js';
import 'monaco-editor/editor/browser/coreCommands.js';
import 'monaco-editor/editor/contrib/caretOperations/browser/caretOperations.js';
import 'monaco-editor/editor/contrib/find/browser/findController.js';
import 'monaco-editor/editor/contrib/gotoError/browser/markerSelectionStatus.js';
import 'monaco-editor/editor/contrib/suggest/browser/suggestController.js';
import 'monaco-editor/editor/common/standaloneStrings.js';
import '../node_modules/monaco-editor/esm/vs/base/browser/ui/codicons/codicon/codicon-modifiers.css';

export { CancellationTokenSource, Emitter, KeyCode, KeyMod, MarkerSeverity, MarkerTag, Position, Range, Selection, SelectionDirection, Token, Uri, editor, languages } from 'monaco-editor/editor/editor.api.js';

