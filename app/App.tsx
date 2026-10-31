import { colorScheme, effect, keymap, mount, Scroll, signal, Storage, Text, View } from "nsh";
import type { InputRef, ViewRef } from "nsh";
import { AlertDialog, Input, Scrollbar } from "nsh/base";
import { Menu } from "nsh/desktop";
import { Library, MarkdownView, NoteEditor, PreviewView, Share, Themes, Updater } from "toolbook/native";

type Note = { path: string; name: string; dir: string };

// No scroll bars anywhere: views still scroll, with no bar drawn.
Scrollbar.setTheme({ vertical: null, horizontal: null });

Library.ensureSeeded();
const notes = signal<Note[]>(Library.list());
const current = signal<string>(notes()[0]?.path ?? "");
type ViewMode = "doc" | "markdown" | "preview";
const view = signal<ViewMode>("doc");
const status = signal("");
const runToken = signal(0);
const selectToken = signal(0);
const filter = signal("");
// Which of the app's own text fields has the focus, for ⌘A.
const focusedInput = signal<"filter" | "quick" | null>(null);
let filterRef: InputRef | null = null;
let quickRef: InputRef | null = null;
const source = signal("");
const quickOpen = signal(false);
const query = signal("");
const theme = signal(Themes.current());

function applyTheme(id: string) {
  Themes.apply(id)
    .then(() => {
      theme.set(id);
      Storage.setItem("theme", id);
    })
    .catch((e: unknown) => status.set(String(e)));
}

Storage.getItem("theme").then((saved) => {
  if (saved) applyTheme(saved);
  else if (colorScheme() === "light") applyTheme("toolbook-lab");
});

function open(path: string) {
  current.set(path);
  quickOpen.set(false);
  query.set("");
}

const dirs = signal<string[]>(Library.dirs());

function refreshLibrary() {
  notes.set(Library.list());
  dirs.set(Library.dirs());
}

/// A new note in category `dir` ("" for the top), opened.
function newNoteIn(dir: string) {
  for (let i = 1; i < 1000; i++) {
    try {
      const path = Library.create(dir, i === 1 ? "未命名" : `未命名-${i}`);
      refreshLibrary();
      open(path);
      return;
    } catch {
      // That name is taken: try the next.
    }
  }
}

/// ⌘N and the sidebar's button: in the open note's category, else 草稿.
function newNote() {
  const dir = notes().find((n) => n.path === current())?.dir;
  newNoteIn(dir || "草稿");
}

// The note or category being renamed in place, and its new name.
const renaming = signal<{ kind: "note" | "dir"; key: string } | null>(null);
const renameText = signal("");

function startRename(kind: "note" | "dir", key: string, name: string) {
  renaming.set({ kind, key });
  renameText.set(name);
}

function cancelRename() {
  renaming.set(null);
}

function commitRename() {
  const r = renaming();
  const name = renameText().trim();
  if (!r || !name) return cancelRename();
  try {
    if (r.kind === "note") {
      const was = current() === r.key;
      const path = Library.rename(r.key, name);
      refreshLibrary();
      if (was) current.set(path);
    } else {
      const old = r.key;
      Library.renameDir(old, name);
      const openNote = notes().find((n) => n.path === current());
      refreshLibrary();
      // The open note moved with its category.
      if (openNote && openNote.dir === old) {
        const moved = notes().find((n) => n.dir === name && n.name === openNote.name);
        if (moved) current.set(moved.path);
      }
    }
    cancelRename();
  } catch (e) {
    status.set(String(e));
  }
}

function moveNote(path: string, dir: string) {
  try {
    const was = current() === path;
    const to = Library.moveTo(path, dir);
    refreshLibrary();
    if (was) current.set(to);
  } catch (e) {
    status.set(String(e));
  }
}

function trash(path: string, what: string) {
  Library.trash(path)
    .then(() => {
      refreshLibrary();
      if (current() === path || current().startsWith(path + "/")) current.set(notes()[0]?.path ?? "");
      status.set(`已将 ${what} 移到废纸篓`);
    })
    .catch((e: unknown) => status.set(String(e)));
}

function noteMenu(n: Note, at: { x: number; y: number }) {
  const others = dirs().filter((d) => d !== n.dir);
  Menu.popup(
    [
      { label: "重命名", onSelect: () => startRename("note", n.path, n.name) },
      {
        label: "移动到",
        submenu: [
          ...(n.dir ? [{ label: "笔记（不分类）", onSelect: () => moveNote(n.path, "") }] : []),
          ...others.map((d) => ({ label: d, onSelect: () => moveNote(n.path, d) })),
        ],
      },
      { type: "separator" },
      { label: "移到废纸篓", onSelect: () => trash(n.path, n.name) },
    ],
    at,
  );
}

function dirMenu(dir: string, at: { x: number; y: number }) {
  Menu.popup(
    [
      { label: "在这里新建笔记", onSelect: () => newNoteIn(dir) },
      { label: "重命名分类", onSelect: () => startRename("dir", dir, dir) },
      { type: "separator" },
      { label: "删除分类（移到废纸篓）", onSelect: () => trash(`${Library.root()}/${dir}`, `分类 ${dir}`) },
    ],
    at,
  );
}

// Dragging: a note onto a category, or a category to another place. A
// move of more than a few points takes the touch from the row's press; the
// drop lands in the category under the pointer (above the first: the
// first; below the last: the last).
const groupRefs = new Map<string, ViewRef>();
const dragging = signal<{ kind: "note" | "dir"; key: string } | null>(null);
const dragOffset = signal(0);
// The category a drag would drop into now, shown highlighted.
const dropDir = signal<string | null>(null);
// The sidebar's categories as shown (set by the sidebar).
let currentGroups: () => string[] = () => [];

/// A view's box, or null when it does not answer soon: a view the sidebar's
/// scroll view keeps out of sight may never answer, and a drop does not wait.
const measureSoon = (ref: ViewRef) =>
  Promise.race([ref.measure(), new Promise<null>((done) => setTimeout(() => done(null), 100))]);

/// The category under page height `y`, with the boxes of all categories in order.
async function dropTarget(y: number) {
  // The categories shown now: a ref left from one no longer shown is skipped.
  const shown = currentGroups();
  const measured = await Promise.all(
    [...groupRefs].filter(([dir]) => shown.includes(dir)).map(async ([dir, ref]) => ({ dir, m: await measureSoon(ref) })),
  );
  const boxes = measured.flatMap(({ dir, m }) =>
    m ? [{ dir, top: m.pagePosition.y, bottom: m.pagePosition.y + m.bounds.size.height }] : [],
  );
  boxes.sort((a, b) => a.top - b.top);
  if (boxes.length === 0) return null;
  const first = boxes[0];
  const last = boxes[boxes.length - 1];
  // Above the first box or below the last: the first or the last category
  // as listed, measured or not.
  const hit =
    boxes.find((b) => y >= b.top && y < b.bottom) ??
    (y < first.top
      ? { dir: shown[0] ?? first.dir, top: first.top, bottom: first.bottom }
      : { dir: shown[shown.length - 1] ?? last.dir, top: last.bottom, bottom: last.bottom + 1 });
  return { hit, boxes };
}

async function drop(y: number) {
  const d = dragging();
  dragging.set(null);
  dragOffset.set(0);
  dropDir.set(null);
  if (!d) return;
  const target = await dropTarget(y);
  if (!target) return;
  if (d.kind === "note") {
    const note = notes().find((n) => n.path === d.key);
    if (note && note.dir !== target.hit.dir) moveNote(d.key, target.hit.dir);
    return;
  }
  // A category goes before the one it was dropped on (after it when
  // dropped on its lower half); the top level ("") is not reordered.
  const onto = target.hit.dir;
  if (onto === d.key) return;
  const order = dirs().filter((x) => x !== d.key);
  let at = onto === "" ? 0 : order.indexOf(onto);
  if (at < 0) at = order.length;
  else if (onto !== "" && y > (target.hit.top + target.hit.bottom) / 2) at += 1;
  order.splice(at, 0, d.key);
  try {
    Library.setDirOrder(order);
    refreshLibrary();
  } catch (e) {
    status.set(String(e));
  }
}

type Gesture = { dy: number; y0: number };
type Touch = { pageY: number };
// Where the touch went down (pointer coordinates), and the dragged row's
// middle measured as the categories are: the drop point is that middle
// moved by how far the pointer went, so both live in one coordinate space.
let dragY0 = 0;
let rowMiddle: Promise<number> = Promise.resolve(0);
let movedBeforeGrant = 0;
const dropY = async (pointerY: number) => (await rowMiddle) + (pointerY - dragY0);

/// A row that can be dragged and pressed: a move of more than a few points
/// takes the touch from the press and drags (following the pointer,
/// dropped where let go).
function Draggable(props: {
  kind: "note" | "dir";
  dragKey: string;
  testID: string;
  children: JSX.Element;
  onPress?: (e: { clickCount: number }) => void;
  onContextMenu?: (e: { pagePosition: { x: number; y: number } }) => void;
  class?: string;
}) {
  const mine = () => dragging()?.key === props.dragKey;
  let row: ViewRef | null = null;
  return (
    <View
      ref={(r: ViewRef) => (row = r)}
      testID={props.testID}
      role={props.onPress ? "button" : undefined}
      onPress={props.onPress}
      onContextMenu={props.onContextMenu}
      class={props.class}
      onMoveShouldSetResponderCapture={(_e: unknown, g: Gesture) => {
        if (Math.abs(g.dy) <= 6) return false;
        // Only a distance from here: the touch events give the positions.
        movedBeforeGrant = g.dy;
        const r = row;
        rowMiddle = r ? r.measure().then((m) => (m ? m.pagePosition.y + m.bounds.size.height / 2 : 0)) : Promise.resolve(0);
        return true;
      }}
      onResponderGrant={(e: Touch) => {
        // Where the touch went down, in the touch events' own coordinates.
        dragY0 = e.pageY - movedBeforeGrant;
        dragging.set({ kind: props.kind, key: props.dragKey });
        dragOffset.set(0);
        // Keeps the sidebar's scroll view from taking the drag.
        return true;
      }}
      onResponderMove={(e: Touch) => {
        dragOffset.set(e.pageY - dragY0);
        dropY(e.pageY)
          .then(dropTarget)
          .then((t) => mine() && dropDir.set(t ? t.hit.dir : null));
      }}
      onResponderRelease={(e: Touch) => {
        dropY(e.pageY).then(drop);
      }}
      onResponderTerminate={() => {
        dragging.set(null);
        dragOffset.set(0);
        dropDir.set(null);
      }}
      style={{
        translate: [0, mine() ? dragOffset() : 0],
        opacity: mine() ? 0.6 : 1,
        zIndex: mine() ? 10 : 0,
      }}
    >
      {props.children}
    </View>
  );
}

const addingDir = signal(false);
const dirName = signal("");

function cancelNewDir() {
  addingDir.set(false);
  dirName.set("");
}

function createDir() {
  const name = dirName().trim();
  if (!name) return cancelNewDir();
  try {
    Library.createDir(name);
    refreshLibrary();
    cancelNewDir();
  } catch (e) {
    status.set(String(e));
  }
}

/// ⌘A: the text of the focused search field, else the whole note.
function selectAll() {
  if (focusedInput() === "filter" && filterRef) filterRef.setSelection(0, filter().length);
  else if (focusedInput() === "quick" && quickRef) quickRef.setSelection(0, query().length);
  else selectToken.set(selectToken() + 1);
}

function exportHtml() {
  Share.exportHtml(current())
    .then((saved) => saved && status.set(`已导出：${saved}`))
    .catch((e: unknown) => status.set(String(e)));
}

function copyMarkdown() {
  try {
    Share.copyMarkdown(current());
    status.set("已复制 Markdown（含结果）");
  } catch (e) {
    status.set(String(e));
  }
}

const noteName = () => notes().find((n) => n.path === current())?.name ?? "";
const matches = () => {
  const q = query().toLowerCase();
  return notes().filter((n) => !q || n.name.toLowerCase().includes(q) || n.dir.toLowerCase().includes(q));
};

// Updates: a look shortly after launch and then once a day (unless turned
// off), and from the app menu. A version the user skipped is not offered by
// the automatic look again.
const autoUpdate = signal(true);
let updating = false;
Storage.getItem("update.auto").then((v) => autoUpdate.set(v !== "off"));

function setAutoUpdate(on: boolean) {
  autoUpdate.set(on);
  Storage.setItem("update.auto", on ? "on" : "off");
}

async function checkForUpdates(manual: boolean) {
  if (updating || (!manual && !autoUpdate())) return;
  updating = true;
  try {
    if (manual) status.set("正在检查更新…");
    const found = await Updater.check();
    if (!found) {
      if (manual) {
        status.set("");
        await AlertDialog.show({
          title: "已是最新版本",
          description: `Toolbook ${Updater.current()} 是目前最新的版本。`,
          actions: [{ id: "ok", label: "好" }],
        });
      }
      return;
    }
    status.set("");
    if (!manual && (await Storage.getItem("update.skip")) === found.version) return;
    const details = found.notes ? `\n\n${found.notes}` : "";
    const { action } = await AlertDialog.show({
      title: `Toolbook ${found.version} 可以更新了`,
      description: `当前版本是 ${Updater.current()}。安装后 Toolbook 会自动重新打开。${details}`,
      actions: [
        { id: "install", label: "安装并重新打开", role: "default" },
        { id: "later", label: "稍后", role: "cancel" },
        { id: "skip", label: "跳过这个版本" },
      ],
      closeOnEscape: true,
    });
    if (action === "skip") Storage.setItem("update.skip", found.version);
    if (action !== "install") return;
    const progress = setInterval(() => status.set(Updater.status()), 300);
    try {
      await Updater.install();
    } finally {
      clearInterval(progress);
    }
    status.set("更新已安装，正在重新打开…");
    Updater.relaunch();
  } catch (e) {
    status.set("");
    // An automatic look that fails (offline) says nothing; one asked for does.
    if (manual) {
      await AlertDialog.show({ title: "更新失败", description: String(e), actions: [{ id: "ok", label: "好" }] });
    }
  } finally {
    updating = false;
  }
}

setTimeout(() => {
  checkForUpdates(false);
  setInterval(() => checkForUpdates(false), 24 * 60 * 60 * 1000);
}, 30 * 1000);

effect(() => {
  const themes = Themes.list();
  const now = theme();
  Menu.setApplicationMenu([
    {
      label: "Toolbook",
      submenu: [
        { role: "about" },
        { label: "检查更新…", onSelect: () => checkForUpdates(true) },
        { label: "自动检查更新", checked: autoUpdate(), onSelect: () => setAutoUpdate(!autoUpdate()) },
        { type: "separator" },
        { role: "hide" },
        { role: "quit" },
      ],
    },
    {
      label: "文件",
      submenu: [
        { label: "新建笔记", shortcut: "mod+n", onSelect: newNote },
        { label: "新建分类", shortcut: "mod+shift+n", onSelect: () => addingDir.set(true) },
        { label: "快速打开…", shortcut: "mod+p", onSelect: () => quickOpen.set(true) },
      ],
    },
    {
      label: "编辑",
      submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }, { label: "全选", shortcut: "mod+a", onSelect: selectAll }],
    },
    {
      label: "视图",
      submenu: [
        { label: "切换 Markdown", shortcut: "mod+/", onSelect: () => view.set(view() === "doc" ? "markdown" : "doc") },
        { label: "预览", shortcut: "mod+shift+p", onSelect: () => view.set("preview") },
      ],
    },
    { label: "运行", submenu: [{ label: "运行全部", shortcut: "mod+enter", onSelect: () => runToken.set(runToken() + 1) }] },
    {
      label: "分享",
      submenu: [
        { label: "导出 HTML…", shortcut: "mod+shift+e", onSelect: exportHtml },
        { label: "复制 Markdown（含结果）", shortcut: "mod+shift+c", onSelect: copyMarkdown },
      ],
    },
    {
      label: "外观",
      submenu: themes.map((t) => ({ label: t.name, checked: t.id === now, onSelect: () => applyTheme(t.id) })),
    },
  ]);
});

/// The app icon (deep-space circuit), drawn with views: a book whose spine
/// is a chain of nodes. No image file to load.
function AppIcon() {
  const node = (left: number, top: number, color: string) => (
    <View style={{ position: "absolute", left, top, width: 5, height: 5, borderRadius: 2.5, backgroundColor: color }} />
  );
  return (
    <View
      accessibilityLabel="Toolbook 图标"
      style={{ width: 24, height: 24, borderRadius: 6, backgroundColor: "#0b0f17", borderWidth: 1, borderColor: "#22314a" }}
    >
      <View style={{ position: "absolute", left: 6, top: 4, width: 12, height: 15, borderRadius: 2, borderWidth: 1.5, borderColor: "#3a4a63" }} />
      <View style={{ position: "absolute", left: 9.5, top: 7, width: 1.5, height: 5, backgroundColor: "#0fd3c3" }} />
      <View style={{ position: "absolute", left: 10, top: 12.5, width: 4.6, height: 1.5, backgroundColor: "#0fd3c3", transform: [{ rotate: "28deg" }] }} />
      <View style={{ position: "absolute", left: 13.6, top: 14, width: 1.5, height: 3.5, backgroundColor: "#0fd3c3" }} />
      {node(7.8, 5.5, "#0fd3c3")}
      {node(7.8, 10.5, "#0fd3c3")}
      {node(12, 12.5, "#ffb547")}
      {node(12, 16, "#0fd3c3")}
    </View>
  );
}

function RenameInput() {
  // Bound while renaming, removed with the field.
  keymap.bind("escape", cancelRename, { when: "always" });
  return (
    <Input
      autoFocus
      testID="rename-input"
      value={renameText()}
      onChange={(e) => renameText.set(e.value)}
      onSubmit={commitRename}
      onBlur={commitRename}
      class="mx-2 px-2 py-1 text-sm rounded-md bg-background border border-primary"
    />
  );
}

function NewDirInput() {
  // Bound while the field is shown, removed with it.
  keymap.bind("escape", cancelNewDir, { when: "always" });
  return (
    <View class="px-3 pb-2">
      <Input
        autoFocus
        placeholder="分类名"
        value={dirName()}
        onChange={(e) => dirName.set(e.value)}
        onSubmit={createDir}
        class="px-2 py-1 text-sm rounded-md bg-background border border-primary"
      />
    </View>
  );
}

function Sidebar() {
  const shown = () => {
    const q = filter().trim().toLowerCase();
    return notes().filter((n) => !q || n.name.toLowerCase().includes(q) || n.dir.toLowerCase().includes(q));
  };
  // Every category, empty ones too; while filtering, those with matches.
  const groups = () => {
    const withNotes = shown().map((n) => n.dir);
    const all = filter().trim() ? withNotes : [...dirs(), ...withNotes];
    // In the library's order (dragged), the top level first.
    const order = dirs();
    return [...new Set(all)].sort((a, b) => {
      if (a === "") return -1;
      if (b === "") return 1;
      const pa = order.indexOf(a), pb = order.indexOf(b);
      return (pa < 0 ? 1e9 : pa) - (pb < 0 ? 1e9 : pb) || a.localeCompare(b, "zh");
    });
  };
  currentGroups = groups;
  return (
    <View class="w-56 bg-sidebar border-r border-sidebar-border pt-10 pb-3 gap-2">
      <View class="flex-row items-center gap-2 px-4">
        <AppIcon />
        <Text class="text-sm font-semibold text-sidebar-primary">Toolbook</Text>
      </View>
      <View class="px-3">
        <Input
          ref={(r) => (filterRef = r)}
          placeholder="筛选笔记"
          value={filter()}
          onChange={(e) => filter.set(e.value)}
          onFocus={() => focusedInput.set("filter")}
          onBlur={() => focusedInput() === "filter" && focusedInput.set(null)}
          class="px-2 py-1 text-sm rounded-md bg-background border border-sidebar-border"
        />
      </View>
      <Scroll class="flex-1">
        {groups().map((dir) => (
          <View
            class={`pb-2 rounded-md ${dragging() && dropDir() === dir ? "bg-sidebar-accent" : ""}`}
            ref={(r: ViewRef) => groupRefs.set(dir, r)}
          >
            <Draggable kind="dir" dragKey={dir} testID={`dir-${dir}`}>
            <View
              class="flex-row items-center pl-4 pr-3 py-1"
              onContextMenu={(e) => dir && dirMenu(dir, e.pagePosition)}
            >
              {renaming()?.kind === "dir" && renaming()?.key === dir ? (
                <View class="flex-1">
                  <RenameInput />
                </View>
              ) : (
                <Text class="flex-1 text-xs text-muted-foreground">{dir || "笔记"}</Text>
              )}
              <View
                role="button"
                testID={`add-${dir}`}
                accessibilityLabel={`在 ${dir || "笔记"} 里新建笔记`}
                onPress={() => newNoteIn(dir)}
                class="w-5 h-5 items-center justify-center rounded-md"
              >
                <Text class="text-sm text-muted-foreground">＋</Text>
              </View>
            </View>
            </Draggable>
            {shown()
              .filter((n) => n.dir === dir)
              .map((n) =>
                renaming()?.kind === "note" && renaming()?.key === n.path ? (
                  <RenameInput />
                ) : (
                  <Draggable
                    kind="note"
                    dragKey={n.path}
                    testID={`note-${n.name}`}
                    onPress={(e) => (e.clickCount >= 2 ? startRename("note", n.path, n.name) : open(n.path))}
                    onContextMenu={(e) => noteMenu(n, e.pagePosition)}
                    class={`mx-2 px-2 py-1 rounded-md ${n.path === current() ? "bg-sidebar-accent" : ""}`}
                  >
                    <Text class={`text-sm ${n.path === current() ? "text-sidebar-primary" : "text-sidebar-foreground"}`}>{n.name}</Text>
                  </Draggable>
                ),
              )}
          </View>
        ))}
        {shown().length === 0 && filter().trim() ? <Text class="px-4 py-2 text-xs text-muted-foreground">没有匹配的笔记</Text> : null}
      </Scroll>
      {addingDir() ? <NewDirInput /> : null}
      <View
        role="button"
        onPress={(e) =>
          Menu.popup(
            [
              { label: "新建笔记", shortcut: "mod+n", onSelect: newNote },
              { label: "新建分类", shortcut: "mod+shift+n", onSelect: () => addingDir.set(true) },
            ],
            e.pagePosition,
          )
        }
        class="mx-2 px-2 py-1 rounded-md"
      >
        <Text class="text-sm text-muted-foreground">＋ 新建</Text>
      </View>
    </View>
  );
}

function TopBar() {
  const seg = (id: ViewMode, label: string) => (
    <View role="button" onPress={() => view.set(id)} class={`px-3 py-1 rounded-md ${view() === id ? "bg-primary" : ""}`}>
      <Text class={`text-xs ${view() === id ? "text-primary-foreground" : "text-muted-foreground"}`}>{label}</Text>
    </View>
  );
  return (
    <View class="flex-row items-center gap-3 px-4 pt-8 pb-2 border-b border-border">
      <Text selectable class="text-base font-semibold text-foreground">{noteName()}</Text>
      <Text class="flex-1 text-xs text-muted-foreground">{status()}</Text>
      <View role="button" onPress={() => quickOpen.set(true)} class="px-2 py-1">
        <Text class="text-xs text-muted-foreground">⌘P 打开</Text>
      </View>
      <View class="flex-row p-1 rounded-lg bg-muted">
        {seg("doc", "文档")}
        {seg("markdown", "Markdown")}
        {seg("preview", "预览")}
      </View>
    </View>
  );
}

function closeQuickOpen() {
  quickOpen.set(false);
  query.set("");
}

function QuickOpen() {
  // Bound while the overlay is shown, removed with it.
  keymap.bind("escape", closeQuickOpen, { when: "always" });
  return (
    <View class="absolute inset-0 items-center pt-24 bg-black/30">
      <View class="w-[480px] rounded-xl bg-popover border border-border p-2 gap-1">
        <Input
          autoFocus
          ref={(r) => (quickRef = r)}
          onFocus={() => focusedInput.set("quick")}
          onBlur={() => focusedInput() === "quick" && focusedInput.set(null)}
          placeholder="搜索笔记"
          value={query()}
          onChange={(e) => query.set(e.value)}
          onSubmit={() => matches()[0] && open(matches()[0].path)}
          class="px-3 py-2 text-sm"
        />
        {matches()
          .slice(0, 12)
          .map((n) => (
            <View role="button" testID={`open-${n.name}`} onPress={() => open(n.path)} class="flex-row gap-3 px-3 py-1 rounded-md">
              <Text class="text-sm text-popover-foreground">{n.name}</Text>
              <Text class="text-xs text-muted-foreground">{n.dir}</Text>
            </View>
          ))}
        <View role="button" onPress={closeQuickOpen} class="px-3 py-1">
          <Text class="text-xs text-muted-foreground">取消</Text>
        </View>
      </View>
    </View>
  );
}

function App() {
  return (
    <View class="flex-1 flex-row bg-background">
      <Sidebar />
      <View class="flex-1">
        <TopBar />
        <View class="flex-1 px-6 py-4">
          {view() === "doc" ? (
            <NoteEditor
              path={current()}
              runToken={runToken()}
              selectToken={selectToken()}
              onStatus={(e) => status.set(e.text)}
              onChanged={(e) => source.set(e.markdown)}
            />
          ) : view() === "markdown" ? (
            <MarkdownView path={current()} runToken={runToken()} selectToken={selectToken()} />
          ) : (
            <View class="flex-1 gap-3">
              <View class="flex-row items-center gap-2">
                <Text class="flex-1 text-xs text-muted-foreground">别人看到的样子：从输入到输出的每一步</Text>
                <View role="button" onPress={copyMarkdown} class="px-3 py-1 rounded-md border border-border">
                  <Text class="text-xs text-foreground">复制 Markdown（含结果）</Text>
                </View>
                <View role="button" onPress={exportHtml} class="px-3 py-1 rounded-md bg-primary">
                  <Text class="text-xs text-primary-foreground">导出 HTML…</Text>
                </View>
              </View>
              <PreviewView path={current()} refreshToken={runToken()} />
            </View>
          )}
        </View>
      </View>
      {quickOpen() ? <QuickOpen /> : null}
    </View>
  );
}

mount(App);
