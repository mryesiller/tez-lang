// Task families for web apps (init / update / view, with subs, fetches and save/restore). A family is
// `(r) => ({ name, desc, code })`; `code` is a complete program with its tests. Every program is proved and its tests
// run by the compiler before it is written out (see build.mjs).
//
// Held out for evaluation (bench/tezbench/web.mjs): the counter with a reset button, the show/hide paragraph, the
// Celsius box, the seconds-since-open stopwatch, the morning/afternoon greeting, the button that loads quote.txt, the
// to-do list with Remove buttons and the page that remembers its visit count. The families below are other ideas.

export const apps = [];
const app = (id, make) => apps.push({ id, make });

const pad = `fn pad(n: Int) -> Str = if n < 10 { "0" ++ n.toStr() } else { n.toStr() }\n`;

app("app.step-counter", (r) => {
  const step = r.pick([2, 3, 5, 10]);
  const max = step * r.int(4, 12);
  const label = r.pick(["Tickets", "Volume", "Items", "Seats", "Level"]);
  return {
    name: `${label.toLowerCase()}-stepper`,
    desc: `A page that shows "${label}: N" and has "-" and "+" buttons plus a "Reset" button. "+" adds ${step} and "-" subtracts ${step}. The value never goes below 0 and never above ${max}.`,
    code: `type Model = { count: Int where it >= 0 && it <= ${max} }

type Msg = Up | Down | Reset

fn init() -> Model = Model { count: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Up => if m.count + ${step} <= ${max} { m with { count: m.count + ${step} } } else { m with { count: ${max} } }
  Down => if m.count >= ${step} { m with { count: m.count - ${step} } } else { m with { count: 0 } }
  Reset => init()
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${label}"),
      Html.paragraph("${label}: " ++ m.count.toStr() ++ " of ${max}"),
      Html.row([Html.button("-", Down), Html.button("+", Up), Html.button("Reset", Reset)])
    ]
  )

test startsAtZero = init().count == 0

test upAddsOneStep = update(init(), Up).count == ${step}

test downStopsAtZero = update(init(), Down).count == 0

test upStopsAtTheLimit = update(update(update(update(update(update(update(update(update(update(update(update(update(init(), Up), Up), Up), Up), Up), Up), Up), Up), Up), Up), Up), Up), Up).count <= ${max}

test resetGoesBack = update(update(init(), Up), Reset).count == 0
`,
  };
});

app("app.votes", (r) => {
  const [a, b] = r.pick([["Like", "Dislike"], ["Yes", "No"], ["Agree", "Disagree"], ["Up", "Down"], ["Cats", "Dogs"], ["Tea", "Coffee"]]);
  return {
    name: `${a.toLowerCase()}-${b.toLowerCase()}-votes`,
    desc: `A voting page with a "${a}" button and a "${b}" button. It shows how many votes each has and the difference (${a} minus ${b}). A "Clear" button resets both to zero.`,
    code: `type Model = { a: Int where it >= 0, b: Int where it >= 0 }

type Msg = VoteA | VoteB | Clear

fn init() -> Model = Model { a: 0, b: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  VoteA => m with { a: m.a + 1 }
  VoteB => m with { b: m.b + 1 }
  Clear => init()
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Vote"),
      Html.row([Html.button("${a}", VoteA), Html.button("${b}", VoteB), Html.button("Clear", Clear)]),
      Html.paragraph("${a}: " ++ m.a.toStr() ++ "   ${b}: " ++ m.b.toStr()),
      Html.paragraph("Difference: " ++ (m.a - m.b).toStr())
    ]
  )

test startsEmpty = init().a == 0 && init().b == 0

test countsEachSide = update(update(update(init(), VoteA), VoteA), VoteB).a == 2

test clearResets = update(update(init(), VoteB), Clear).b == 0
`,
  };
});

app("app.theme-switch", (r) => {
  const [lightName, darkName] = r.pick([["Light", "Dark"], ["Day", "Night"], ["Bright", "Dim"]]);
  const text = r.pick(["Welcome back", "Reading mode", "Settings", "My notes"]);
  return {
    name: "theme-switch",
    desc: `A page with the heading "${text}" and a button that switches between a ${lightName.toLowerCase()} look and a ${darkName.toLowerCase()} look (change the background and text colour with a style). The button says which mode will be switched to. A Reset button goes back to the ${lightName.toLowerCase()} look.`,
    code: `type Model = { dark: Bool }

type Msg = Switch | Reset

fn init() -> Model = Model { dark: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Switch => m with { dark: !m.dark }
  Reset => init()
}

fn look(m: Model) -> Str =
  if m.dark { "background: #1e1e1e; color: #eeeeee; padding: 16px" } else { "background: #ffffff; color: #222222; padding: 16px" }

fn view(m: Model) -> Html[Msg] =
  Html.node(
    "div",
    [UiProp("style", look(m))],
    [
      Html.heading("${text}"),
      Html.row([Html.button(if m.dark { "Switch to ${lightName}" } else { "Switch to ${darkName}" }, Switch), Html.button("Reset", Reset)])
    ]
  )

test startsLight = !init().dark

test switchesBack = !update(update(init(), Switch), Switch).dark

test darkLooksDark = look(update(init(), Switch)) != look(init())

test resetGoesBackToLight = !update(update(init(), Switch), Reset).dark
`,
  };
});

app("app.name-greeter", (r) => {
  const hello = r.pick(["Hello", "Welcome", "Hi there", "Good to see you"]);
  const prompt = r.pick(["Please type your name.", "Who are you?", "Enter a name above."]);
  return {
    name: "name-greeter",
    desc: `A page with a text box for a name. Below it, show "${hello}, <name>!" using the trimmed name, or "${prompt}" while the box is blank.`,
    code: `type Model = { name: Str }

type Msg = SetName(Str)

fn init() -> Model = Model { name: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  SetName(s) => m with { name: s }
}

fn greeting(name: Str) -> Str =
  if name.trim().isEmpty() { "${prompt}" } else { "${hello}, " ++ name.trim() ++ "!" }

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.input(m.name, s => SetName(s)), Html.paragraph(greeting(m.name))])

test blankAsks = greeting("   ") == "${prompt}"

test greetsByName = greeting(" Ada ") == "${hello}, Ada!"

test typingStoresTheText = update(init(), SetName("Bo")).name == "Bo"
`,
  };
});

app("app.text-stats", (r) => {
  const heading = r.pick(["Text counter", "Writing stats", "Count my text"]);
  return {
    name: "text-stats",
    desc: `A page titled "${heading}" with a text box. It shows the number of characters and the number of words (words are separated by spaces; extra spaces do not count).`,
    code: `type Model = { text: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { text: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { text: s }
}

fn words(s: Str) -> Int = s.split(" ").filter(w => !w.isEmpty()).len()

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${heading}"),
      Html.input(m.text, s => Typed(s)),
      Html.paragraph("Characters: " ++ m.text.len().toStr()),
      Html.paragraph("Words: " ++ words(m.text).toStr())
    ]
  )

test countsWords = words("one  two three") == 3

test emptyHasNoWords = words("") == 0

test spacesAloneAreNotWords = words("   ") == 0

test remembersText = update(init(), Typed("hi")).text == "hi"
`,
  };
});

app("app.echo-transform", (r) => {
  const kind = r.pick(["upper", "lower", "reverse"]);
  const spec = {
    upper: ["capital letters", "s.upper()", "UPPER CASE"],
    lower: ["lower case", "s.lower()", "lower case"],
    reverse: ["reversed", `s.chars().reverse().join("")`, "Backwards"],
  }[kind];
  return {
    name: `${kind}-echo`,
    desc: `A page with a text box that shows what was typed in ${spec[0]} underneath, under the heading "${spec[2]}".`,
    code: `type Model = { text: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { text: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { text: s }
}

fn transform(s: Str) -> Str = ${spec[1]}

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.heading("${spec[2]}"), Html.input(m.text, s => Typed(s)), Html.paragraph(transform(m.text))])

test transforms = transform("Tez Lang") == ${kind === "upper" ? '"TEZ LANG"' : kind === "lower" ? '"tez lang"' : '"gnaL zeT"'}

test emptyStaysEmpty = transform("") == ""
`,
  };
});

app("app.unit-converter", (r) => {
  const u = r.pick([
    ["Kilometres", "Miles", "km", "miles", "n * 621 / 1000"],
    ["Kilograms", "Pounds", "kg", "lb", "n * 22 / 10"],
    ["Centimetres", "Inches", "cm", "in", "n * 39 / 100"],
    ["Litres", "Gallons", "l", "gal", "n * 264 / 1000"],
    ["Metres", "Feet", "m", "ft", "n * 328 / 100"],
    ["Hours", "Minutes", "h", "min", "n * 60"],
  ]);
  const f = (n) => eval(u[4].replace(/n/g, n).replace(/(\d+) \* (\d+) \/ (\d+)/, "Math.trunc($1 * $2 / $3)"));
  return {
    name: `${u[2]}-to-${u[3]}`,
    desc: `A converter page: the visitor types a whole number of ${u[0].toLowerCase()} and the page shows the value in ${u[1].toLowerCase()} (${u[4].replace("n", "the number")}, rounded down). Show "Type a whole number." when the box does not hold one.`,
    code: `type Model = { input: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { input: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { input: s }
}

fn convert(n: Int) -> Int = ${u[4]}

fn answer(s: Str) -> Str = match s.trim().toInt() {
  Some(n) => n.toStr() ++ " ${u[2]} = " ++ convert(n).toStr() ++ " ${u[3]}"
  None => "Type a whole number."
}

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.heading("${u[0]} to ${u[1]}"), Html.input(m.input, s => Typed(s)), Html.paragraph(answer(m.input))])

test converts = convert(100) == ${Math.trunc(f(100))}

test zeroStaysZero = convert(0) == 0

test answersNumbers = answer(" 10 ") == "10 ${u[2]} = ${Math.trunc(f(10))} ${u[3]}"

test rejectsText = answer("abc") == "Type a whole number."
`,
  };
});

app("app.tip-calculator", (r) => {
  const percents = r.pick([[10, 15, 20], [5, 10, 15], [12, 15, 18], [10, 20, 25]]);
  return {
    name: "tip-calculator",
    desc: `A tip calculator: a text box for the bill (whole currency units) and three buttons for ${percents.map((p) => p + "%").join(", ")}. It shows the tip (rounded down) and the total. A bill that is not a number counts as 0.`,
    code: `type Model = { bill: Str, percent: Int where it >= 0 }

type Msg = Bill(Str) | Pick(Int)

fn init() -> Model = Model { bill: "", percent: ${percents[1]} }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Bill(s) => m with { bill: s }
  Pick(p) => if p >= 0 { m with { percent: p } } else { m }
}

fn amount(s: Str) -> Int = {
  let n = s.trim().toInt().unwrapOr(0)
  if n < 0 { 0 } else { n }
}

fn tip(bill: Int, percent: Int) -> Int = bill * percent / 100

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Tip calculator"),
      Html.input(m.bill, s => Bill(s)),
      Html.row([${percents.map((p) => `Html.button("${p}%", Pick(${p}))`).join(", ")}]),
      Html.paragraph("Tip: " ++ tip(amount(m.bill), m.percent).toStr()),
      Html.paragraph("Total: " ++ (amount(m.bill) + tip(amount(m.bill), m.percent)).toStr())
    ]
  )

test computesTheTip = tip(200, ${percents[0]}) == ${Math.trunc((200 * percents[0]) / 100)}

test roundsDown = tip(99, ${percents[1]}) == ${Math.trunc((99 * percents[1]) / 100)}

test badBillIsZero = amount("abc") == 0 && amount("-5") == 0

test pickingChangesThePercent = update(init(), Pick(${percents[2]})).percent == ${percents[2]}
`,
  };
});

app("app.countdown", (r) => {
  const start = r.pick([10, 15, 30, 45, 60, 90, 120]);
  const what = r.pick(["Timer", "Countdown", "Break timer", "Focus timer"]);
  return {
    name: "countdown-timer",
    desc: `A "${what}" starting at ${start} seconds with Start, Stop and Reset buttons. While it is running it counts down once per second and stops by itself at 0. It shows the seconds left.`,
    code: `type Model = { left: Int where it >= 0, running: Bool }

type Msg = Start | Stop | Reset | Tick

fn init() -> Model = Model { left: ${start}, running: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Start => if m.left > 0 { m with { running: true } } else { m }
  Stop => m with { running: false }
  Reset => init()
  Tick => if m.running && m.left > 0 { Model { left: m.left - 1, running: m.left > 1 } } else { m }
}

fn subs(m: Model) -> List[Sub[Msg]] = if m.running { [Sub.every(1000, Tick)] } else { [] }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${what}"),
      Html.paragraph(m.left.toStr() ++ " seconds left"),
      Html.row([Html.button("Start", Start), Html.button("Stop", Stop), Html.button("Reset", Reset)])
    ]
  )

test doesNotTickWhenStopped = update(init(), Tick).left == ${start}

test ticksWhileRunning = update(update(init(), Start), Tick).left == ${start - 1}

test onlySubscribesWhileRunning = subs(init()).isEmpty() && subs(update(init(), Start)).len() == 1

test resetRestores = update(update(update(init(), Start), Tick), Reset).left == ${start}
`,
  };
});

app("app.digital-clock", (r) => {
  const label = r.pick(["UTC time", "Server clock", "Clock", "The time is"]);
  return {
    name: "digital-clock",
    desc: `A clock page titled "${label}" that shows the wall-clock time as HH:MM:SS (UTC, 24 hours) and updates every second.`,
    code: `type Model = { now: Int }

type Msg = Tick(Int)

fn init() -> Model = Model { now: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Tick(t) => m with { now: t }
}

${pad}
fn clockText(t: Int) -> Str = pad(t / 3600 % 24) ++ ":" ++ pad(t / 60 % 60) ++ ":" ++ pad(t % 60)

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(1000, t => Tick(t))]

fn view(m: Model) -> Html[Msg] = Html.col([Html.heading("${label}"), Html.paragraph(clockText(m.now))])

test formatsMidnight = clockText(0) == "00:00:00"

test formatsAfternoon = clockText(3600 * 15 + 60 * 4 + 9) == "15:04:09"

test wrapsAfterADay = clockText(86400 + 61) == "00:01:01"

test followsTheClock = update(init(), Tick(3661)).now == 3661
`,
  };
});

app("app.open-closed", (r) => {
  const open = r.int(7, 10);
  const close = r.int(16, 21);
  const shop = r.pick(["Bakery", "Library", "Clinic", "Café", "Studio"]);
  return {
    name: "opening-hours",
    desc: `A page for a ${shop.toLowerCase()} that shows "Open now" between ${open}:00 and ${close}:00 UTC (closing time excluded) and "Closed, opens at ${open}:00" otherwise. It follows the wall clock, checking once a minute.`,
    code: `type Model = { now: Int }

type Msg = Tick(Int)

fn init() -> Model = Model { now: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Tick(t) => m with { now: t }
}

fn hour(t: Int) -> Int = t / 3600 % 24

fn isOpen(t: Int) -> Bool = hour(t) >= ${open} && hour(t) < ${close}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(60000, t => Tick(t))]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${shop}"),
      Html.paragraph(if isOpen(m.now) { "Open now" } else { "Closed, opens at ${open}:00" })
    ]
  )

test closedAtNight = !isOpen(3600 * 3)

test openAtOpening = isOpen(3600 * ${open})

test closedAtClosing = !isOpen(3600 * ${close})

test openMidday = isOpen(3600 * 12 + 600)
`,
  };
});

app("app.online-badge", (r) => {
  const label = r.pick(["Connection status", "Network", "Are we online?"]);
  return {
    name: "online-badge",
    desc: `A page titled "${label}" that shows whether the browser is online or offline and whether this tab is visible or hidden, updating when either changes.`,
    code: `type Model = { online: Bool, visible: Bool }

type Msg = Net(Bool) | Seen(Bool)

fn init() -> Model = Model { online: true, visible: true }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Net(b) => m with { online: b }
  Seen(b) => m with { visible: b }
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.online(b => Net(b)), Sub.visible(b => Seen(b))]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${label}"),
      Html.paragraph(if m.online { "You are online." } else { "You are offline." }),
      Html.paragraph(if m.visible { "This tab is in front." } else { "This tab is hidden." })
    ]
  )

test startsOnline = init().online && init().visible

test noticesOffline = !update(init(), Net(false)).online

test noticesHidden = !update(init(), Seen(false)).visible

test listensToBoth = subs(init()).len() == 2
`,
  };
});

app("app.tabs", (r) => {
  const sets = [
    ["Overview", "Details", "Reviews", "A short summary.", "All the specifications.", "What people say."],
    ["Home", "Menu", "Contact", "Welcome to our place.", "Today's dishes.", "Write to us any time."],
    ["Profile", "Security", "Billing", "Your public profile.", "Passwords and sign-in.", "Invoices and plans."],
    ["Intro", "Steps", "Tips", "Why this matters.", "Do these in order.", "Common mistakes."],
  ];
  const s = r.pick(sets);
  return {
    name: "three-tabs",
    desc: `A page with three tab buttons: ${s[0]}, ${s[1]} and ${s[2]}. Clicking one shows its text: "${s[3]}", "${s[4]}" and "${s[5]}". The first tab is shown at the start; the active tab's button is marked with a class.`,
    code: `type Tab = First | Second | Third

type Model = { tab: Tab }

type Msg = Show(Tab)

fn init() -> Model = Model { tab: First }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Show(t) => m with { tab: t }
}

fn body(t: Tab) -> Str = match t {
  First => "${s[3]}"
  Second => "${s[4]}"
  Third => "${s[5]}"
}

fn tabButton(m: Model, t: Tab, label: Str) -> Html[Msg] =
  Html.node(
    "button",
    [UiClick(Show(t)), UiProp("class", if m.tab == t { "tab active" } else { "tab" })],
    [Html.text(label)]
  )

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.row([tabButton(m, First, "${s[0]}"), tabButton(m, Second, "${s[1]}"), tabButton(m, Third, "${s[2]}")]),
      Html.paragraph(body(m.tab))
    ]
  )

test startsOnTheFirstTab = body(init().tab) == "${s[3]}"

test switchesTab = body(update(init(), Show(Third)).tab) == "${s[5]}"

test canGoBack = update(update(init(), Show(Second)), Show(First)).tab == First
`,
  };
});

app("app.shopping-list", (r) => {
  const title = r.pick(["Shopping list", "Packing list", "Wish list", "Reading list"]);
  return {
    name: "simple-list",
    desc: `A "${title}" page: a text box and an Add button that appends the trimmed text as a new line (blank text is ignored and the box is cleared after adding). A Clear button empties the list. It shows how many entries there are.`,
    code: `type Model = { draft: Str, items: List[Str] }

type Msg = Draft(Str) | Add | Clear

fn init() -> Model = Model { draft: "", items: [] }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Draft(s) => m with { draft: s }
  Add => if m.draft.trim().isEmpty() { m } else { Model { draft: "", items: m.items.append(m.draft.trim()) } }
  Clear => m with { items: [] }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.row([Html.input(m.draft, s => Draft(s)), Html.button("Add", Add), Html.button("Clear", Clear)]),
      Html.node("ul", [], m.items.map(i => Html.node("li", [], [Html.text(i)]))),
      Html.paragraph(m.items.len().toStr() ++ " entries")
    ]
  )

test addsTrimmedText = update(update(init(), Draft("  milk ")), Add).items == ["milk"]

test ignoresBlank = update(update(init(), Draft("   ")), Add).items.isEmpty()

test clearsTheBoxAfterAdding = update(update(init(), Draft("eggs")), Add).draft == ""

test keepsOrder = update(update(update(update(init(), Draft("a")), Add), Draft("b")), Add).items == ["a", "b"]

test clearEmptiesTheList = update(update(update(init(), Draft("a")), Add), Clear).items.isEmpty()
`,
  };
});

app("app.notes-newest-first", (r) => {
  const title = r.pick(["Quick notes", "Journal", "Ideas", "Daily log"]);
  const limit = r.pick([3, 5, 8]);
  return {
    name: "notes-newest-first",
    desc: `A "${title}" page: type a note and press Save; notes are listed newest first and only the latest ${limit} are kept. Blank notes are ignored. Show how many notes are kept.`,
    code: `type Model = { draft: Str, notes: List[Str] }

type Msg = Draft(Str) | Save

fn init() -> Model = Model { draft: "", notes: [] }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Draft(s) => m with { draft: s }
  Save => if m.draft.trim().isEmpty() { m } else {
    Model { draft: "", notes: [m.draft.trim()].concat(m.notes).take(${limit}) }
  }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.row([Html.input(m.draft, s => Draft(s)), Html.button("Save", Save)]),
      Html.node("ul", [], m.notes.map(n => Html.node("li", [], [Html.text(n)]))),
      Html.paragraph(m.notes.len().toStr() ++ " kept")
    ]
  )

fn saved(words: List[Str]) -> Model = words.fold(init(), (m, w) => update(update(m, Draft(w)), Save))

test newestFirst = saved(["one", "two"]).notes == ["two", "one"]

test keepsOnlyTheLatest = saved(${JSON.stringify(Array.from({ length: limit + 2 }, (_, i) => `n${i + 1}`))}).notes.len() == ${limit}

test blankIsIgnored = saved(["  "]).notes.isEmpty()
`,
  };
});

app("app.load-on-open", (r) => {
  const file = r.pick(["news", "tips", "status", "notice", "motd", "changelog"]);
  const title = r.pick(["Latest news", "Tip of the day", "Service status", "Notice board", "Message of the day", "What is new"]);
  return {
    name: `load-${file}`,
    desc: `A page titled "${title}" that loads the text file ${file}.txt from this site as soon as it opens and shows it ("Loading..." first, then the text, or "Could not load it." if the request fails). A Refresh button loads it again.`,
    code: `type Model = { version: Int where it >= 0, text: Str, failed: Bool }

type Msg = Got(Str) | Failed(Str) | Refresh

fn init() -> Model = Model { version: 0, text: "", failed: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Got(t) => m with { text: t, failed: false }
  Failed(e) => m with { failed: true }
  Refresh => Model { version: m.version + 1, text: "", failed: false }
}

fn fetches(m: Model) -> List[Fetch[Msg]] =
  [Fetch.get("${file}.txt?v=" ++ m.version.toStr(), t => Got(t), e => Failed(e))]

fn shown(m: Model) -> Str =
  if m.failed { "Could not load it." } else if m.text.isEmpty() { "Loading..." } else { m.text }

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.heading("${title}"), Html.paragraph(shown(m)), Html.button("Refresh", Refresh)])

test startsLoading = shown(init()) == "Loading..."

test showsTheText = shown(update(init(), Got("hello"))) == "hello"

test showsFailure = shown(update(init(), Failed("HTTP 404"))) == "Could not load it."

test refreshAsksAgain = fetches(update(init(), Refresh)).len() == 1 && update(init(), Refresh).version == 1
`,
  };
});

app("app.auto-refresh", (r) => {
  const file = r.pick(["scores", "prices", "headlines", "stock", "weather", "queue"]);
  const secs = r.pick([5, 10, 15, 30, 60]);
  const title = r.pick(["Live scores", "Prices", "Headlines", "Stock levels", "Weather", "Queue length"]);
  return {
    name: `auto-refresh-${file}`,
    desc: `A page titled "${title}" that shows the text of ${file}.txt from this site and reloads it by itself every ${secs} seconds, so the content stays current without a click. Show "Loading..." until the first answer and keep the last text if a reload fails.`,
    code: `type Model = { round: Int where it >= 0, text: Str }

type Msg = Got(Str) | Failed(Str) | Again

fn init() -> Model = Model { round: 0, text: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Got(t) => m with { text: t }
  Failed(e) => m
  Again => m with { round: m.round + 1 }
}

// a new address each round makes the page ask again
fn fetches(m: Model) -> List[Fetch[Msg]] =
  [Fetch.get("${file}.txt?round=" ++ m.round.toStr(), t => Got(t), e => Failed(e))]

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.every(${secs * 1000}, Again)]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.paragraph(if m.text.isEmpty() { "Loading..." } else { m.text }),
      Html.paragraph("Updated " ++ m.round.toStr() ++ " time(s) since opening, every ${secs} seconds.")
    ]
  )

test keepsTheLastTextOnFailure = update(update(init(), Got("a")), Failed("x")).text == "a"

test asksOnceForTheFile = fetches(init()).len() == 1 && fetches(update(init(), Again)).len() == 1

test schedulesTheNextRound = subs(init()).len() == 1 && update(init(), Again).round == 1

test newTextReplacesOld = update(update(init(), Got("a")), Got("b")).text == "b"
`,
  };
});

app("app.minute-refresh", (r) => {
  const file = r.pick(["report", "summary", "digest", "board"]);
  return {
    name: `minute-refresh-${file}`,
    desc: `A page that shows the text of ${file}.txt and reloads it whenever the wall-clock minute changes (a cron-like schedule driven by the clock), but not while the tab is hidden. Show the time of the last reload as HH:MM.`,
    code: `type Model = { now: Int, minute: Int where it >= 0, round: Int where it >= 0, text: Str, visible: Bool }

type Msg = Clock(Int) | Seen(Bool) | Got(Str) | Failed(Str)

fn init() -> Model = Model { now: 0, minute: 0, round: 0, text: "", visible: true }

${pad}
fn hhmm(t: Int) -> Str = pad(t / 3600 % 24) ++ ":" ++ pad(t / 60 % 60)

fn update(m: Model, msg: Msg) -> Model = match msg {
  Clock(t) => if t >= 0 && t / 60 != m.minute && m.visible {
    Model { now: t, minute: t / 60, round: m.round + 1, text: m.text, visible: m.visible }
  } else { m with { now: t } }
  Seen(v) => m with { visible: v }
  Got(t) => m with { text: t }
  Failed(e) => m
}

fn fetches(m: Model) -> List[Fetch[Msg]] =
  if m.round > 0 { [Fetch.get("${file}.txt?r=" ++ m.round.toStr(), t => Got(t), e => Failed(e))] } else { [] }

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(1000, t => Clock(t)), Sub.visible(v => Seen(v))]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Report"),
      Html.paragraph(if m.text.isEmpty() { "Waiting for the next minute..." } else { m.text }),
      Html.paragraph("Last reload at " ++ hhmm(m.now) ++ " UTC")
    ]
  )

test reloadsWhenTheMinuteChanges = update(init(), Clock(125)).round == 1

test staysQuietWithinAMinute = update(update(init(), Clock(125)), Clock(150)).round == 1

test hiddenTabsDoNotReload = update(update(init(), Seen(false)), Clock(125)).round == 0

test formatsTheTime = hhmm(3600 * 7 + 60 * 3) == "07:03"

test asksForTheFileAfterTheFirstReload = fetches(update(init(), Clock(125))).len() == 1 && fetches(init()).isEmpty()
`,
  };
});

app("app.saved-note", (r) => {
  const title = r.pick(["Scratchpad", "Reminder", "Sticky note", "Draft"]);
  return {
    name: "saved-note",
    desc: `A "${title}" page with one text box. What is typed is saved in the browser and comes back when the page is opened again. Show how many characters are saved.`,
    code: `type Model = { text: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { text: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { text: s }
}

// what the browser keeps, and how it comes back
fn save(m: Model) -> Str = m.text

fn restore(s: Str) -> Option[Model] = Some(Model { text: s })

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.input(m.text, s => Typed(s)),
      Html.paragraph(m.text.len().toStr() ++ " characters saved")
    ]
  )

test whatIsSavedComesBack = match restore(save(update(init(), Typed("remember me")))) {
  Some(m) => m.text == "remember me"
  None => false
}

test typingChangesTheText = update(init(), Typed("x")).text == "x"
`,
  };
});

app("app.saved-preference", (r) => {
  const [a, b] = r.pick([["Compact", "Comfortable"], ["Small", "Large"], ["List", "Grid"]]);
  return {
    name: "saved-preference",
    desc: `A page with a button that switches between "${a}" and "${b}" display (it just shows which one is chosen). The choice is remembered in the browser; an unknown saved value is ignored and the page starts with "${a}". A Reset button goes back to "${a}".`,
    code: `type Model = { second: Bool }

type Msg = Flip | Reset

fn init() -> Model = Model { second: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Flip => m with { second: !m.second }
  Reset => init()
}

fn save(m: Model) -> Str = if m.second { "${b}" } else { "${a}" }

fn restore(s: Str) -> Option[Model] =
  if s == "${a}" { Some(Model { second: false }) } else if s == "${b}" { Some(Model { second: true }) } else { None }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Display"),
      Html.paragraph("Showing: " ++ save(m)),
      Html.row([Html.button("Switch", Flip), Html.button("Reset", Reset)])
    ]
  )

test remembersTheChoice = match restore(save(update(init(), Flip))) {
  Some(m) => m.second
  None => false
}

test ignoresUnknownValues = restore("nonsense").isNone()

test startsWithTheFirst = save(init()) == "${a}"

test resetGoesBack = save(update(update(init(), Flip), Reset)) == "${a}"
`,
  };
});

app("app.goal-clicks", (r) => {
  const goal = r.pick([5, 10, 20, 25, 50]);
  const thing = r.pick(["Push-ups", "Glasses of water", "Pages read", "Laps", "Words written"]);
  return {
    name: "goal-tracker",
    desc: `A tracker for "${thing}": a button adds one, a progress bar shows progress toward a goal of ${goal}, the text shows "n / ${goal}", and "Goal reached!" appears at ${goal}. The count stops at the goal. A Restart button starts over.`,
    code: `type Model = { done: Int where it >= 0 && it <= ${goal} }

type Msg = AddOne | Restart

fn init() -> Model = Model { done: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  AddOne => if m.done < ${goal} { m with { done: m.done + 1 } } else { m }
  Restart => init()
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${thing}"),
      Html.node("progress", [UiProp("value", m.done.toStr()), UiProp("max", "${goal}")], []),
      Html.paragraph(m.done.toStr() ++ " / ${goal}"),
      if m.done == ${goal} { Html.paragraph("Goal reached!") } else { Html.text("") },
      Html.row([Html.button("+1", AddOne), Html.button("Restart", Restart)])
    ]
  )

fn times(n: Int, m: Model) -> Model = if n <= 0 { m } else { times(n - 1, update(m, AddOne)) }

test countsUp = update(update(init(), AddOne), AddOne).done == 2

test stopsAtTheGoal = times(${goal + 3}, init()).done == ${goal}

test restartStartsOver = update(times(3, init()), Restart).done == 0
`,
  };
});

app("app.rating", (r) => {
  const max = r.pick([3, 5, 5, 10]);
  const thing = r.pick(["Rate this recipe", "How was your stay?", "Rate the talk", "Your score"]);
  return {
    name: "rating",
    desc: `A rating page titled "${thing}" with buttons 1 to ${max}. It shows the chosen rating as stars drawn with "*" and "-" (for example "***--" for 3 of 5) and the number. Nothing is chosen at first.`,
    code: `type Model = { rating: Int where it >= 0 && it <= ${max} }

type Msg = Rate(Int)

fn init() -> Model = Model { rating: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Rate(n) => if n >= 0 && n <= ${max} { m with { rating: n } } else { m }
}

fn stars(n: Int) -> Str = "*".repeat(n.clamp(0, ${max})) ++ "-".repeat(${max} - n.clamp(0, ${max}))

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${thing}"),
      Html.row(List.range(1, ${max + 1}).map(n => Html.button(n.toStr(), Rate(n)))),
      Html.paragraph(stars(m.rating) ++ "  (" ++ m.rating.toStr() ++ ")")
    ]
  )

test nothingChosenAtFirst = stars(init().rating) == ${JSON.stringify("-".repeat(max))}

test drawsStars = stars(3) == ${JSON.stringify("***" + "-".repeat(max - 3))}

test ratingIsStored = update(init(), Rate(2)).rating == 2

test outOfRangeIsIgnored = update(init(), Rate(${max + 4})).rating == 0
`,
  };
});

app("app.cart-two", (r) => {
  const [a, pa] = r.pick([["Coffee", 3], ["Tea", 2], ["Bagel", 4], ["Juice", 5]]);
  const [b, pb] = r.pick([["Cake", 6], ["Cookie", 2], ["Muffin", 4], ["Salad", 7]]);
  return {
    name: "cart-two-items",
    desc: `A small order page with two items: ${a} at ${pa} and ${b} at ${pb}. Each has "-" and "+" buttons for the quantity (never below 0) and the page shows the quantities and the total price.`,
    code: `type Model = { a: Int where it >= 0, b: Int where it >= 0 }

type Msg = MoreA | LessA | MoreB | LessB

fn init() -> Model = Model { a: 0, b: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  MoreA => m with { a: m.a + 1 }
  LessA => if m.a > 0 { m with { a: m.a - 1 } } else { m }
  MoreB => m with { b: m.b + 1 }
  LessB => if m.b > 0 { m with { b: m.b - 1 } } else { m }
}

fn total(m: Model) -> Int = m.a * ${pa} + m.b * ${pb}

fn line(name: Str, qty: Int, less: Msg, more: Msg) -> Html[Msg] =
  Html.row([Html.text(name ++ " x" ++ qty.toStr()), Html.button("-", less), Html.button("+", more)])

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Order"),
      line("${a}", m.a, LessA, MoreA),
      line("${b}", m.b, LessB, MoreB),
      Html.paragraph("Total: " ++ total(m).toStr())
    ]
  )

test emptyOrderCostsNothing = total(init()) == 0

test addsUpPrices = total(update(update(update(init(), MoreA), MoreA), MoreB)) == ${2 * pa + pb}

test quantityNeverNegative = update(init(), LessB).b == 0
`,
  };
});

app("app.age-form", (r) => {
  const [t1, t2] = [r.int(12, 14), r.int(17, 19)];
  return {
    name: "age-check",
    desc: `A page with a box for an age. It says "Child" below ${t1}, "Teenager" below ${t2}, otherwise "Adult". If the box does not hold a whole number from 0 to 120 it says "Enter an age from 0 to 120."`,
    code: `type Model = { age: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { age: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { age: s }
}

fn verdict(s: Str) -> Str = match s.trim().toInt() {
  Some(n) => if n < 0 || n > 120 { "Enter an age from 0 to 120." } else if n < ${t1} { "Child" } else if n < ${t2} { "Teenager" } else { "Adult" }
  None => "Enter an age from 0 to 120."
}

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.heading("Age check"), Html.input(m.age, s => Typed(s)), Html.paragraph(verdict(m.age))])

test child = verdict("${t1 - 1}") == "Child"

test teenager = verdict("${t1}") == "Teenager"

test adult = verdict("${t2}") == "Adult"

test rejectsNonsense = verdict("abc") == "Enter an age from 0 to 120." && verdict("200") == "Enter an age from 0 to 120."
`,
  };
});

app("app.dice-by-clock", (r) => {
  const sides = r.pick([6, 6, 8, 10, 20]);
  return {
    name: "clock-dice",
    desc: `A dice page: pressing Roll shows a number from 1 to ${sides} taken from the current second of the wall clock (seconds modulo ${sides}, plus 1). It shows "Press Roll" before the first roll.`,
    code: `type Model = { now: Int, roll: Int where it >= 0 && it <= ${sides} }

type Msg = Tick(Int) | Roll

fn init() -> Model = Model { now: 0, roll: 0 }

fn face(t: Int) -> Int = if t < 0 { 1 } else { t % ${sides} + 1 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Tick(t) => m with { now: t }
  Roll => m with { roll: face(m.now) }
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(1000, t => Tick(t))]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Dice"),
      Html.paragraph(if m.roll == 0 { "Press Roll" } else { "You rolled " ++ m.roll.toStr() }),
      Html.button("Roll", Roll)
    ]
  )

test facesStayInRange = face(0) == 1 && face(${sides - 1}) == ${sides} && face(${sides}) == 1

test rollsFromTheClock = update(update(init(), Tick(${sides + 2})), Roll).roll == 3

test notRolledYet = init().roll == 0
`,
  };
});

app("app.pagination", (r) => {
  const pages = r.int(3, 6);
  const noun = r.pick(["Chapter", "Slide", "Step", "Page"]);
  return {
    name: "pager",
    desc: `A pager over ${pages} ${noun.toLowerCase()}s: shows "${noun} p of ${pages}" with Previous and Next buttons that never leave 1..${pages}.`,
    code: `type Model = { page: Int where it >= 1 && it <= ${pages} }

type Msg = Next | Previous

fn init() -> Model = Model { page: 1 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Next => if m.page < ${pages} { m with { page: m.page + 1 } } else { m }
  Previous => if m.page > 1 { m with { page: m.page - 1 } } else { m }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${noun} " ++ m.page.toStr() ++ " of ${pages}"),
      Html.row([Html.button("Previous", Previous), Html.button("Next", Next)])
    ]
  )

test startsAtOne = init().page == 1

test cannotGoBeforeOne = update(init(), Previous).page == 1

test stopsAtTheEnd = update(update(update(update(update(update(update(init(), Next), Next), Next), Next), Next), Next), Next).page <= ${pages}

test movesForwardAndBack = update(update(init(), Next), Previous).page == 1
`,
  };
});

app("app.progress-bar", (r) => {
  const step = r.pick([5, 10, 20, 25]);
  const thing = r.pick(["Upload", "Download", "Install", "Backup"]);
  return {
    name: "progress-bar",
    desc: `A "${thing}" page with a progress bar from 0 to 100 percent. "Advance" adds ${step} (never past 100), "Back" subtracts ${step} (never below 0), and the text shows the percentage and "Done" at 100.`,
    code: `type Model = { percent: Int where it >= 0 && it <= 100 }

type Msg = Advance | Back

fn init() -> Model = Model { percent: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Advance => if m.percent + ${step} <= 100 { m with { percent: m.percent + ${step} } } else { m with { percent: 100 } }
  Back => if m.percent >= ${step} { m with { percent: m.percent - ${step} } } else { m with { percent: 0 } }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${thing}"),
      Html.node("progress", [UiProp("value", m.percent.toStr()), UiProp("max", "100")], []),
      Html.paragraph(m.percent.toStr() ++ "%" ++ if m.percent == 100 { " Done" } else { "" }),
      Html.row([Html.button("Back", Back), Html.button("Advance", Advance)])
    ]
  )

fn advanceTimes(n: Int, m: Model) -> Model = if n <= 0 { m } else { advanceTimes(n - 1, update(m, Advance)) }

test advancesByAStep = update(init(), Advance).percent == ${step}

test neverPastAHundred = advanceTimes(${Math.ceil(100 / step) + 2}, init()).percent == 100

test neverBelowZero = update(init(), Back).percent == 0
`,
  };
});

app("app.color-picker", (r) => {
  const colors = r.pick([
    [["Red", "#d32f2f"], ["Green", "#388e3c"], ["Blue", "#1976d2"]],
    [["Sunset", "#ff7043"], ["Forest", "#2e7d32"], ["Ocean", "#0277bd"]],
    [["Rose", "#e91e63"], ["Mint", "#26a69a"], ["Gold", "#fbc02d"]],
  ]);
  return {
    name: "color-picker",
    desc: `A colour picker with three buttons (${colors.map((c) => c[0]).join(", ")}). A square swatch below shows the chosen colour (${colors.map((c) => `${c[0]} is ${c[1]}`).join(", ")}) and the name is written under it. Start with ${colors[0][0]}.`,
    code: `type Choice = One | Two | Three

type Model = { choice: Choice }

type Msg = Pick(Choice)

fn init() -> Model = Model { choice: One }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Pick(c) => m with { choice: c }
}

fn nameOf(c: Choice) -> Str = match c {
  One => "${colors[0][0]}"
  Two => "${colors[1][0]}"
  Three => "${colors[2][0]}"
}

fn hexOf(c: Choice) -> Str = match c {
  One => "${colors[0][1]}"
  Two => "${colors[1][1]}"
  Three => "${colors[2][1]}"
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.row([Html.button(nameOf(One), Pick(One)), Html.button(nameOf(Two), Pick(Two)), Html.button(nameOf(Three), Pick(Three))]),
      Html.node("div", [UiProp("style", "width: 80px; height: 80px; background: " ++ hexOf(m.choice))], []),
      Html.paragraph(nameOf(m.choice))
    ]
  )

test startsWithTheFirst = nameOf(init().choice) == "${colors[0][0]}"

test picksTheThird = hexOf(update(init(), Pick(Three)).choice) == "${colors[2][1]}"

test eachChoiceHasItsOwnColour = hexOf(One) != hexOf(Two) && hexOf(Two) != hexOf(Three)
`,
  };
});

app("app.flashcards", (r) => {
  const decks = [
    [["Capital of France?", "Paris"], ["2 + 2 * 2?", "6"], ["Opposite of hot?", "Cold"]],
    [["Bonjour means?", "Hello"], ["Gracias means?", "Thank you"], ["Ciao means?", "Bye"]],
    [["HTML stands for?", "HyperText Markup Language"], ["What does CSS style?", "Web pages"], ["Is 7 prime?", "Yes"]],
  ];
  const d = r.pick(decks);
  return {
    name: "flashcards",
    desc: "A flashcard page with three cards. It shows the question of the current card; a Flip button shows or hides the answer, and a Next button moves to the next card, wrapping around to the first after the last (and hiding the answer).",
    code: `type Model = { i: Int where it >= 0 && it <= 2, shown: Bool }

type Msg = Flip | Next

fn init() -> Model = Model { i: 0, shown: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Flip => m with { shown: !m.shown }
  Next => Model { i: if m.i >= 2 { 0 } else { m.i + 1 }, shown: false }
}

fn question(i: Int) -> Str = match i {
  0 => "${d[0][0]}"
  1 => "${d[1][0]}"
  _ => "${d[2][0]}"
}

fn answer(i: Int) -> Str = match i {
  0 => "${d[0][1]}"
  1 => "${d[1][1]}"
  _ => "${d[2][1]}"
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading(question(m.i)),
      Html.paragraph(if m.shown { answer(m.i) } else { "(tap Flip to see the answer)" }),
      Html.row([Html.button("Flip", Flip), Html.button("Next", Next)])
    ]
  )

test startsHidden = !init().shown && init().i == 0

test flipShowsTheAnswer = update(init(), Flip).shown

test nextHidesAgain = !update(update(init(), Flip), Next).shown

test wrapsAround = update(update(update(init(), Next), Next), Next).i == 0
`,
  };
});

app("app.reading-log", (r) => {
  const unit = r.pick(["°C", "kg", "mmHg", "km"]);
  const title = r.pick(["Readings", "Measurements", "Daily values", "Log"]);
  return {
    name: "readings-stats",
    desc: `A "${title}" page: a text box and an Add button that appends a whole number (text that is not a number is ignored). It shows how many readings there are and their lowest, highest and average (rounded down), each followed by "${unit}"; with no readings it says "No readings yet."`,
    code: `type Model = { draft: Str, readings: List[Int] }

type Msg = Draft(Str) | Add

fn init() -> Model = Model { draft: "", readings: [] }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Draft(s) => m with { draft: s }
  Add => match m.draft.trim().toInt() {
    Some(n) => Model { draft: "", readings: m.readings.append(n) }
    None => m
  }
}

fn average(xs: List[Int]) -> Int = if xs.isEmpty() { 0 } else { xs.sum() / xs.len() }

fn summary(xs: List[Int]) -> Str =
  if xs.isEmpty() { "No readings yet." } else {
    xs.len().toStr() ++ " readings: low " ++ xs.min().unwrapOr(0).toStr() ++ "${unit}, high " ++ xs.max().unwrapOr(0).toStr() ++ "${unit}, average " ++ average(xs).toStr() ++ "${unit}"
  }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.row([Html.input(m.draft, s => Draft(s)), Html.button("Add", Add)]),
      Html.paragraph(summary(m.readings))
    ]
  )

fn added(xs: List[Str]) -> Model = xs.fold(init(), (m, s) => update(update(m, Draft(s)), Add))

test emptySaysSo = summary([]) == "No readings yet."

test averagesRoundDown = average([1, 2]) == 1 && average([10, 20, 30]) == 20

test ignoresText = added(["4", "abc", "6"]).readings == [4, 6]

test noReadingsAverageIsZero = average([]) == 0
`,
  };
});

app("app.blinker", (r) => {
  const [a, b] = r.pick([["tick", "tock"], ["ping", "pong"], ["on", "off"], ["left", "right"]]);
  const ms = r.pick([500, 1000, 1500]);
  return {
    name: "blinker",
    desc: `A page that alternates between the words "${a}" and "${b}" by itself every ${ms} milliseconds, with a Pause/Resume button that stops and restarts the blinking.`,
    code: `type Model = { first: Bool, running: Bool }

type Msg = Beat | Toggle

fn init() -> Model = Model { first: true, running: true }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Beat => if m.running { m with { first: !m.first } } else { m }
  Toggle => m with { running: !m.running }
}

fn subs(m: Model) -> List[Sub[Msg]] = if m.running { [Sub.every(${ms}, Beat)] } else { [] }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading(if m.first { "${a}" } else { "${b}" }),
      Html.button(if m.running { "Pause" } else { "Resume" }, Toggle)
    ]
  )

test startsWithTheFirstWord = init().first

test beatSwitchesWords = !update(init(), Beat).first

test pausedDoesNotBlink = update(update(init(), Toggle), Beat).first

test onlySubscribesWhenRunning = subs(init()).len() == 1 && subs(update(init(), Toggle)).isEmpty()
`,
  };
});

app("app.scoreboard", (r) => {
  const [a, b] = r.pick([["Home", "Away"], ["Red", "Blue"], ["Team A", "Team B"], ["Lions", "Tigers"]]);
  return {
    name: "scoreboard",
    desc: `A scoreboard for ${a} and ${b}. Each team has +1, +2 and +3 buttons; the page shows both scores and who is leading ("Tied" when equal). A New game button sets both scores to 0.`,
    code: `type Model = { a: Int where it >= 0, b: Int where it >= 0 }

type Msg = ScoreA(Int) | ScoreB(Int) | NewGame

fn init() -> Model = Model { a: 0, b: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  ScoreA(n) => if n > 0 { m with { a: m.a + n } } else { m }
  ScoreB(n) => if n > 0 { m with { b: m.b + n } } else { m }
  NewGame => init()
}

fn leader(m: Model) -> Str =
  if m.a > m.b { "${a} leads" } else if m.b > m.a { "${b} leads" } else { "Tied" }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${a} " ++ m.a.toStr() ++ " : " ++ m.b.toStr() ++ " ${b}"),
      Html.paragraph(leader(m)),
      Html.row([Html.button("${a} +1", ScoreA(1)), Html.button("${a} +2", ScoreA(2)), Html.button("${a} +3", ScoreA(3))]),
      Html.row([Html.button("${b} +1", ScoreB(1)), Html.button("${b} +2", ScoreB(2)), Html.button("${b} +3", ScoreB(3))]),
      Html.button("New game", NewGame)
    ]
  )

test startsTied = leader(init()) == "Tied"

test addsPoints = update(update(init(), ScoreA(3)), ScoreA(2)).a == 5

test leaderChanges = leader(update(update(init(), ScoreA(2)), ScoreB(3))) == "${b} leads"

test negativePointsAreIgnored = update(init(), ScoreA(-5)).a == 0

test newGameResets = update(update(init(), ScoreB(2)), NewGame).b == 0
`,
  };
});

app("app.login-form", (r) => {
  const minUser = r.int(3, 5);
  const minPass = r.int(6, 10);
  return {
    name: "login-form",
    desc: `A sign-in form with a username box and a password box and a Sign in button. The username needs at least ${minUser} characters and the password at least ${minPass}. Below the form show the first problem ("Username too short", "Password too short") or, if both are fine, "Ready to sign in". Pressing Sign in with a valid form shows "Welcome, <username>!"; otherwise nothing happens.`,
    code: `type Model = { user: Str, pass: Str, welcome: Str }

type Msg = User(Str) | Pass(Str) | SignIn

fn init() -> Model = Model { user: "", pass: "", welcome: "" }

fn problem(user: Str, pass: Str) -> Str =
  if user.trim().len() < ${minUser} { "Username too short" } else if pass.len() < ${minPass} { "Password too short" } else { "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  User(s) => m with { user: s, welcome: "" }
  Pass(s) => m with { pass: s, welcome: "" }
  SignIn => if problem(m.user, m.pass).isEmpty() { m with { welcome: "Welcome, " ++ m.user.trim() ++ "!" } } else { m }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Sign in"),
      Html.input(m.user, s => User(s)),
      Html.input(m.pass, s => Pass(s)),
      Html.button("Sign in", SignIn),
      Html.paragraph(if !m.welcome.isEmpty() { m.welcome } else if problem(m.user, m.pass).isEmpty() { "Ready to sign in" } else { problem(m.user, m.pass) })
    ]
  )

test shortUsername = problem("${"a".repeat(minUser - 1)}", "${"p".repeat(minPass)}") == "Username too short"

test shortPassword = problem("${"a".repeat(minUser)}", "${"p".repeat(minPass - 1)}") == "Password too short"

test validForm = problem("${"a".repeat(minUser)}", "${"p".repeat(minPass)}").isEmpty()

test welcomesValidUsers = update(update(update(init(), User("${"a".repeat(minUser)}")), Pass("${"p".repeat(minPass)}")), SignIn).welcome == "Welcome, ${"a".repeat(minUser)}!"

test refusesInvalidUsers = update(init(), SignIn).welcome.isEmpty()
`,
  };
});

app("app.quote-rotator", (r) => {
  const qs = r.pick([
    ["Stay curious.", "Make it work, then make it right.", "Small steps add up."],
    ["Less is more.", "Done is better than perfect.", "Ask why five times."],
    ["Measure twice, cut once.", "Simple beats clever.", "Write it down."],
  ]);
  const secs = r.pick([4, 5, 8, 10]);
  return {
    name: "quote-rotator",
    desc: `A page that shows one of three quotes and moves to the next one by itself every ${secs} seconds, wrapping around. Previous and Next buttons also work. The quotes are: "${qs[0]}", "${qs[1]}", "${qs[2]}".`,
    code: `type Model = { i: Int where it >= 0 && it <= 2 }

type Msg = Next | Previous

fn init() -> Model = Model { i: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Next => if m.i >= 2 { m with { i: 0 } } else { m with { i: m.i + 1 } }
  Previous => if m.i <= 0 { m with { i: 2 } } else { m with { i: m.i - 1 } }
}

fn quote(i: Int) -> Str = match i {
  0 => "${qs[0]}"
  1 => "${qs[1]}"
  _ => "${qs[2]}"
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.every(${secs * 1000}, Next)]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading(quote(m.i)),
      Html.row([Html.button("Previous", Previous), Html.button("Next", Next)])
    ]
  )

test startsWithTheFirstQuote = quote(init().i) == "${qs[0]}"

test nextWrapsAround = update(update(update(init(), Next), Next), Next).i == 0

test previousWrapsAround = update(init(), Previous).i == 2

test rotatesByItself = subs(init()).len() == 1
`,
  };
});

app("app.calculator", (r) => {
  const ops = r.pick([["+", "-", "*"], ["+", "-", "/"], ["+", "*", "/"]]);
  const hasDiv = ops.includes("/");
  const opName = { "+": "Add", "-": "Subtract", "*": "Multiply", "/": "Divide" };
  return {
    name: "two-number-calculator",
    desc: `A calculator with two text boxes for whole numbers and buttons ${ops.map((o) => `"${o}"`).join(", ")}. The page shows the result of the last operation, or "Enter two whole numbers." if either box is not a number${hasDiv ? ', or "Cannot divide by zero." for a division by zero (division rounds toward zero)' : ""}.`,
    code: `type Op = ${ops.map((o) => opName[o]).join(" | ")}

type Model = { a: Str, b: Str, result: Str }

type Msg = SetA(Str) | SetB(Str) | Apply(Op)

fn init() -> Model = Model { a: "", b: "", result: "" }

fn compute(op: Op, x: Int, y: Int) -> Str = match op {
${ops
  .map((o) => (o === "/" ? `  Divide => if y == 0 { "Cannot divide by zero." } else { (x / y).toStr() }` : `  ${opName[o]} => (x ${o} y).toStr()`))
  .join("\n")}
}

fn answer(op: Op, a: Str, b: Str) -> Str = match a.trim().toInt() {
  Some(x) => match b.trim().toInt() {
    Some(y) => compute(op, x, y)
    None => "Enter two whole numbers."
  }
  None => "Enter two whole numbers."
}

fn update(m: Model, msg: Msg) -> Model = match msg {
  SetA(s) => m with { a: s }
  SetB(s) => m with { b: s }
  Apply(op) => m with { result: answer(op, m.a, m.b) }
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Calculator"),
      Html.row([Html.input(m.a, s => SetA(s)), Html.input(m.b, s => SetB(s))]),
      Html.row([${ops.map((o) => `Html.button("${o}", Apply(${opName[o]}))`).join(", ")}]),
      Html.paragraph(if m.result.isEmpty() { "Result appears here." } else { m.result })
    ]
  )

test firstOperation = answer(${opName[ops[0]]}, "6", "3") == "${ops[0] === "+" ? 9 : 0}"

test needsNumbers = answer(${opName[ops[0]]}, "x", "3") == "Enter two whole numbers."

test appliesTheOperation = update(update(update(init(), SetA("8")), SetB("2")), Apply(${opName[ops[1]]})).result == "${ops[1] === "-" ? 6 : 16}"
${hasDiv ? `\ntest refusesZeroDivision = answer(Divide, "5", "0") == "Cannot divide by zero."\n` : ""}`,
  };
});

app("app.checklist", (r) => {
  const sets = [
    ["Pack passport", "Book hotel", "Buy tickets"],
    ["Write intro", "Add examples", "Proofread"],
    ["Stretch", "Run", "Cool down"],
    ["Open window", "Water plants", "Take out trash"],
  ];
  const s = r.pick(sets);
  return {
    name: "checklist",
    desc: `A checklist with three tasks (${s.map((x) => `"${x}"`).join(", ")}). Clicking a task's button toggles it done; the page shows "done of 3" and "All done!" when every task is done.`,
    code: `type Model = { a: Bool, b: Bool, c: Bool }

type Msg = ToggleA | ToggleB | ToggleC

fn init() -> Model = Model { a: false, b: false, c: false }

fn update(m: Model, msg: Msg) -> Model = match msg {
  ToggleA => m with { a: !m.a }
  ToggleB => m with { b: !m.b }
  ToggleC => m with { c: !m.c }
}

fn doneCount(m: Model) -> Int = [m.a, m.b, m.c].count(x => x)

fn task(label: Str, done: Bool, msg: Msg) -> Html[Msg] =
  Html.button((if done { "[x] " } else { "[ ] " }) ++ label, msg)

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Checklist"),
      task("${s[0]}", m.a, ToggleA),
      task("${s[1]}", m.b, ToggleB),
      task("${s[2]}", m.c, ToggleC),
      Html.paragraph(doneCount(m).toStr() ++ " of 3" ++ if doneCount(m) == 3 { " - All done!" } else { "" })
    ]
  )

test nothingDoneAtFirst = doneCount(init()) == 0

test countsDoneTasks = doneCount(update(update(init(), ToggleA), ToggleC)) == 2

test togglingTwiceUndoes = doneCount(update(update(init(), ToggleB), ToggleB)) == 0

test allDone = doneCount(update(update(update(init(), ToggleA), ToggleB), ToggleC)) == 3
`,
  };
});

// ---------------------------------------------------------------------------------------------- more apps
app("app.habit-week", (r) => {
  const habit = r.pick(["Drink water", "Read 20 minutes", "Go for a walk", "Practice piano", "Write a journal line"]);
  return {
    name: "habit-week",
    desc: `A habit tracker for "${habit}" with seven day buttons (Mon to Sun). Clicking a day marks it done or not done; the page shows "n of 7 days" and "Great week!" when all seven are done.`,
    code: `type Model = { days: List[Bool] }

type Msg = Flip(Int)

fn init() -> Model = Model { days: List.repeat(false, 7) }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Flip(i) => m with { days: m.days.enumerate().map(p => if p.0 == i { !p.1 } else { p.1 }) }
}

fn doneDays(m: Model) -> Int = m.days.count(d => d)

fn dayName(i: Int) -> Str = match i {
  0 => "Mon"
  1 => "Tue"
  2 => "Wed"
  3 => "Thu"
  4 => "Fri"
  5 => "Sat"
  _ => "Sun"
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${habit}"),
      Html.row(m.days.enumerate().map(p => Html.button((if p.1 { "[x] " } else { "[ ] " }) ++ dayName(p.0), Flip(p.0)))),
      Html.paragraph(doneDays(m).toStr() ++ " of 7 days" ++ if doneDays(m) == 7 { " - Great week!" } else { "" })
    ]
  )

test startsEmpty = doneDays(init()) == 0 && init().days.len() == 7

test marksADay = doneDays(update(init(), Flip(2))) == 1

test flippingTwiceUndoes = doneDays(update(update(init(), Flip(4)), Flip(4))) == 0

test aPositionOutsideTheWeekChangesNothing = doneDays(update(init(), Flip(9))) == 0
`,
  };
});

app("app.bmi", (r) => {
  const [lo, hi] = [r.pick([18, 19]), r.pick([24, 25])];
  return {
    name: "bmi-calculator",
    desc: `A body-mass-index page with two text boxes, weight in kilograms and height in centimetres (whole numbers). It shows the index (weight * 10000 / height / height, rounded down) and the category: "Under" below ${lo}, "Healthy" up to ${hi}, otherwise "Over". If either box is not a positive whole number it says "Enter your weight and height."`,
    code: `type Model = { weight: Str, height: Str }

type Msg = Weight(Str) | Height(Str)

fn init() -> Model = Model { weight: "", height: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Weight(s) => m with { weight: s }
  Height(s) => m with { height: s }
}

fn index(kg: Int, cm: Int) -> Int
  requires cm > 0
  = kg * 10000 / cm / cm

fn category(i: Int) -> Str = if i < ${lo} { "Under" } else if i <= ${hi} { "Healthy" } else { "Over" }

fn report(w: Str, h: Str) -> Str = match w.trim().toInt() {
  Some(kg) => match h.trim().toInt() {
    Some(cm) => if kg > 0 && cm > 0 { "BMI " ++ index(kg, cm).toStr() ++ ": " ++ category(index(kg, cm)) } else { "Enter your weight and height." }
    None => "Enter your weight and height."
  }
  None => "Enter your weight and height."
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("BMI"),
      Html.row([Html.input(m.weight, s => Weight(s)), Html.input(m.height, s => Height(s))]),
      Html.paragraph(report(m.weight, m.height))
    ]
  )

test computesTheIndex = index(70, 175) == 22

test categories = category(${lo - 1}) == "Under" && category(${hi}) == "Healthy" && category(${hi + 1}) == "Over"

test reportsAHealthyPerson = report("70", "175") == "BMI 22: ${22 < lo ? "Under" : 22 <= hi ? "Healthy" : "Over"}"

test rejectsZeroAndText = report("0", "170") == "Enter your weight and height." && report("x", "170") == "Enter your weight and height."
`,
  };
});

app("app.currency", (r) => {
  const rates = r.pick([
    [["EUR", 92], ["GBP", 79], ["JPY", 15000]],
    [["USD", 108], ["CAD", 146], ["CHF", 97]],
    [["USD", 100], ["EUR", 92], ["INR", 8300]],
  ]);
  return {
    name: "currency-converter",
    desc: `A converter from a base currency: a text box for an amount and three buttons for ${rates.map((x) => x[0]).join(", ")}. The result is amount * rate / 100 rounded down, with rates ${rates.map((x) => `${x[0]} ${x[1]}`).join(", ")} (per 100 units). It shows "Type a whole number." for a bad amount.`,
    code: `type Currency = First | Second | Third

type Model = { amount: Str, to: Currency }

type Msg = Amount(Str) | Pick(Currency)

fn init() -> Model = Model { amount: "", to: First }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Amount(s) => m with { amount: s }
  Pick(c) => m with { to: c }
}

fn code(c: Currency) -> Str = match c {
  First => "${rates[0][0]}"
  Second => "${rates[1][0]}"
  Third => "${rates[2][0]}"
}

fn rate(c: Currency) -> Int = match c {
  First => ${rates[0][1]}
  Second => ${rates[1][1]}
  Third => ${rates[2][1]}
}

fn converted(c: Currency, n: Int) -> Int = n * rate(c) / 100

fn answer(m: Model) -> Str = match m.amount.trim().toInt() {
  Some(n) => n.toStr() ++ " = " ++ converted(m.to, n).toStr() ++ " " ++ code(m.to)
  None => "Type a whole number."
}

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Currency"),
      Html.input(m.amount, s => Amount(s)),
      Html.row([Html.button(code(First), Pick(First)), Html.button(code(Second), Pick(Second)), Html.button(code(Third), Pick(Third))]),
      Html.paragraph(answer(m))
    ]
  )

test convertsWithTheRate = converted(Second, 200) == ${Math.trunc((200 * rates[1][1]) / 100)}

test answersForTheChosenCurrency = answer(update(update(init(), Amount("100")), Pick(Third))) == "100 = ${rates[2][1]} ${rates[2][0]}"

test rejectsText = answer(update(init(), Amount("abc"))) == "Type a whole number."
`,
  };
});

app("app.char-limit", (r) => {
  const limit = r.pick([50, 80, 140, 200, 280]);
  const title = r.pick(["Post a status", "Write a bio", "Short message", "Tweet-sized note"]);
  return {
    name: "char-limit",
    desc: `A "${title}" page with a text box and a counter "n left" showing how many of ${limit} characters remain. Beyond the limit the counter shows "over by k" instead, and the Post button text becomes "Too long" (it is "Post" otherwise).`,
    code: `type Model = { text: Str }

type Msg = Typed(Str)

fn init() -> Model = Model { text: "" }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Typed(s) => m with { text: s }
}

fn left(s: Str) -> Int = ${limit} - s.len()

fn counter(s: Str) -> Str = if left(s) >= 0 { left(s).toStr() ++ " left" } else { "over by " ++ (0 - left(s)).toStr() }

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${title}"),
      Html.input(m.text, s => Typed(s)),
      Html.paragraph(counter(m.text)),
      Html.node("button", [UiProp("disabled", if left(m.text) < 0 { "true" } else { "false" })], [Html.text(if left(m.text) < 0 { "Too long" } else { "Post" })])
    ]
  )

test emptyHasTheWholeLimit = counter("") == "${limit} left"

test countsDown = counter("hello") == "${limit - 5} left"

test warnsWhenOver = counter("${"x".repeat(limit + 3)}") == "over by 3"

test exactlyAtTheLimitIsFine = counter("${"x".repeat(limit)}") == "0 left"
`,
  };
});

app("app.slideshow", (r) => {
  const names = r.pick([["beach", "forest", "city"], ["cat", "dog", "bird"], ["spring", "summer", "autumn"]]);
  const secs = r.pick([3, 4, 5, 6]);
  return {
    name: "slideshow",
    desc: `A slideshow of three pictures (files ${names.map((n) => n + ".png").join(", ")} next to the page, each with an alt text). It moves to the next picture by itself every ${secs} seconds, wrapping around, and has Previous and Next buttons. A caption shows "n / 3".`,
    code: `type Model = { i: Int where it >= 0 && it <= 2 }

type Msg = Next | Previous

fn init() -> Model = Model { i: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Next => if m.i >= 2 { m with { i: 0 } } else { m with { i: m.i + 1 } }
  Previous => if m.i <= 0 { m with { i: 2 } } else { m with { i: m.i - 1 } }
}

fn picture(i: Int) -> Str = match i {
  0 => "${names[0]}"
  1 => "${names[1]}"
  _ => "${names[2]}"
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.every(${secs * 1000}, Next)]

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.node("img", [UiProp("src", picture(m.i) ++ ".png"), UiProp("alt", "A picture of " ++ picture(m.i))], []),
      Html.paragraph((m.i + 1).toStr() ++ " / 3"),
      Html.row([Html.button("Previous", Previous), Html.button("Next", Next)])
    ]
  )

test startsWithTheFirst = picture(init().i) == "${names[0]}"

test wrapsForward = update(update(update(init(), Next), Next), Next).i == 0

test wrapsBackward = update(init(), Previous).i == 2

test advancesByItself = subs(init()).len() == 1
`,
  };
});

app("app.poll-percent", (r) => {
  const q = r.pick([["Best season?", "Summer", "Winter", "Spring"], ["Favourite language?", "Rust", "Tez", "Elm"], ["Lunch?", "Pizza", "Salad", "Soup"]]);
  return {
    name: "poll",
    desc: `A poll "${q[0]}" with three options (${q[1]}, ${q[2]}, ${q[3]}). Clicking an option adds a vote. For each option the page shows its votes and its share of all votes in whole percent (0% for everything before the first vote), plus a progress bar.`,
    code: `type Model = { a: Int where it >= 0, b: Int where it >= 0, c: Int where it >= 0 }

type Msg = VoteA | VoteB | VoteC

fn init() -> Model = Model { a: 0, b: 0, c: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  VoteA => m with { a: m.a + 1 }
  VoteB => m with { b: m.b + 1 }
  VoteC => m with { c: m.c + 1 }
}

fn total(m: Model) -> Int = m.a + m.b + m.c

fn share(votes: Int, all: Int) -> Int = if all <= 0 { 0 } else { votes * 100 / all }

fn option(label: Str, votes: Int, all: Int, msg: Msg) -> Html[Msg] =
  Html.col(
    [
      Html.button(label, msg),
      Html.node("progress", [UiProp("value", share(votes, all).toStr()), UiProp("max", "100")], []),
      Html.text(votes.toStr() ++ " votes, " ++ share(votes, all).toStr() ++ "%")
    ]
  )

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("${q[0]}"),
      option("${q[1]}", m.a, total(m), VoteA),
      option("${q[2]}", m.b, total(m), VoteB),
      option("${q[3]}", m.c, total(m), VoteC)
    ]
  )

test noVotesMeansZeroPercent = share(0, 0) == 0

test sharesAddUp = share(1, 4) == 25 && share(3, 4) == 75

test votingChangesTheTotal = total(update(update(init(), VoteA), VoteC)) == 2

test roundsDown = share(1, 3) == 33
`,
  };
});

app("app.date-countdown", (r) => {
  const event = r.pick(["Launch day", "New year", "Product release", "Holiday", "The big match"]);
  const target = r.pick([1893456000, 1798761600, 1767225600, 1830297600]);
  return {
    name: "date-countdown",
    desc: `A page titled "${event}" counting down to the moment ${target} (seconds since 1970) using the wall clock, updating every second. It shows days, hours, minutes and seconds left, and "It is time!" once the moment has passed.`,
    code: `type Model = { now: Int }

type Msg = Tick(Int)

fn init() -> Model = Model { now: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Tick(t) => m with { now: t }
}

fn remaining(now: Int) -> Int = if now >= ${target} { 0 } else { ${target} - now }

fn text(now: Int) -> Str = {
  let left = remaining(now)
  if left == 0 { "It is time!" } else {
    (left / 86400).toStr() ++ " days " ++ (left / 3600 % 24).toStr() ++ " h " ++ (left / 60 % 60).toStr() ++ " min " ++ (left % 60).toStr() ++ " s"
  }
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(1000, t => Tick(t))]

fn view(m: Model) -> Html[Msg] = Html.col([Html.heading("${event}"), Html.paragraph(text(m.now))])

test countsDown = text(${target - 90061}) == "1 days 1 h 1 min 1 s"

test endsAtTheMoment = text(${target}) == "It is time!"

test staysFinishedAfterwards = text(${target + 5000}) == "It is time!"

test followsTheClock = update(init(), Tick(77)).now == 77
`,
  };
});

app("app.weekday-message", (r) => {
  const msgs = r.pick([
    ["Fresh start!", "Keep going.", "Midweek push.", "Almost there.", "Friday!", "Relax.", "Recharge."],
    ["Plan the week.", "Deep work day.", "Review day.", "Ship it.", "Wrap up.", "Explore.", "Rest."],
  ]);
  return {
    name: "weekday-message",
    desc: `A page that shows a different message for each day of the week, taken from the wall clock (UTC), refreshing every minute: Monday "${msgs[0]}", Tuesday "${msgs[1]}", Wednesday "${msgs[2]}", Thursday "${msgs[3]}", Friday "${msgs[4]}", Saturday "${msgs[5]}", Sunday "${msgs[6]}". It also shows the day name.`,
    code: `type Model = { now: Int }

type Msg = Tick(Int)

fn init() -> Model = Model { now: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Tick(t) => m with { now: t }
}

// day 0 of the clock (1 January 1970) was a Thursday; 0 = Monday ... 6 = Sunday
fn weekday(t: Int) -> Int = if t < 0 { 0 } else { (t / 86400 + 3) % 7 }

fn dayName(d: Int) -> Str = match d {
  0 => "Monday"
  1 => "Tuesday"
  2 => "Wednesday"
  3 => "Thursday"
  4 => "Friday"
  5 => "Saturday"
  _ => "Sunday"
}

fn message(d: Int) -> Str = match d {
  0 => "${msgs[0]}"
  1 => "${msgs[1]}"
  2 => "${msgs[2]}"
  3 => "${msgs[3]}"
  4 => "${msgs[4]}"
  5 => "${msgs[5]}"
  _ => "${msgs[6]}"
}

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.clock(60000, t => Tick(t))]

fn view(m: Model) -> Html[Msg] =
  Html.col([Html.heading(dayName(weekday(m.now))), Html.paragraph(message(weekday(m.now)))])

test epochWasAThursday = dayName(weekday(0)) == "Thursday"

test nextDayIsFriday = dayName(weekday(86400)) == "Friday"

test weekWrapsAround = weekday(86400 * 7) == weekday(0)

test mondayMessage = message(0) == "${msgs[0]}"
`,
  };
});

app("app.status-page", (r) => {
  const file = r.pick(["status", "health", "uptime", "services"]);
  const secs = r.pick([15, 30, 60]);
  return {
    name: "status-page",
    desc: `A status page that loads ${file}.txt from this site and reloads it every ${secs} seconds. It shows the text, "Loading..." at first, and keeps the last text if a reload fails but then adds "(last update failed)". It also shows "You are offline" while the browser is offline.`,
    code: `type Model = { round: Int where it >= 0, text: Str, failed: Bool, online: Bool }

type Msg = Got(Str) | Failed(Str) | Again | Net(Bool)

fn init() -> Model = Model { round: 0, text: "", failed: false, online: true }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Got(t) => m with { text: t, failed: false }
  Failed(e) => m with { failed: true }
  Again => m with { round: m.round + 1 }
  Net(b) => m with { online: b }
}

fn fetches(m: Model) -> List[Fetch[Msg]] =
  [Fetch.get("${file}.txt?round=" ++ m.round.toStr(), t => Got(t), e => Failed(e))]

fn subs(m: Model) -> List[Sub[Msg]] = [Sub.every(${secs * 1000}, Again), Sub.online(b => Net(b))]

fn shown(m: Model) -> Str =
  (if m.text.isEmpty() { "Loading..." } else { m.text }) ++ (if m.failed && !m.text.isEmpty() { " (last update failed)" } else { "" })

fn view(m: Model) -> Html[Msg] =
  Html.col(
    [
      Html.heading("Status"),
      Html.paragraph(shown(m)),
      if m.online { Html.text("") } else { Html.paragraph("You are offline") }
    ]
  )

test startsLoading = shown(init()) == "Loading..."

test showsTheText = shown(update(init(), Got("All systems go"))) == "All systems go"

test keepsTheOldTextOnFailure = shown(update(update(init(), Got("OK")), Failed("x"))) == "OK (last update failed)"

test notesOffline = !update(init(), Net(false)).online

test reloadsOnASchedule = subs(init()).len() == 2
`,
  };
});

// ---------------------------------------------------------------------------------------------- remembered in the browser
// Half of these apps come with the program's own `save` and `restore`; a restored value is checked like any other
// (the compiler proves the restored model keeps its refinements).
const num = (code, re) => Number(re.exec(code)?.[1]);
const persistSpecs = {
  "app.step-counter": (t) => {
    const max = num(t.code, /it <= (\d+)/);
    return { what: "The count", block: `fn save(m: Model) -> Str = m.count.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(n) => if n >= 0 && n <= ${max} { Some(Model { count: n }) } else { None }\n  None => None\n}`, tests: "test restoresWhatWasSaved = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"many\").isNone()" };
  },
  "app.goal-clicks": (t) => {
    const max = num(t.code, /it <= (\d+)/);
    return { what: "The progress", block: `fn save(m: Model) -> Str = m.done.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(n) => if n >= 0 && n <= ${max} { Some(Model { done: n }) } else { None }\n  None => None\n}`, tests: "test restoresWhatWasSaved = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"-4\").isNone()" };
  },
  "app.rating": (t) => {
    const max = num(t.code, /it <= (\d+)/);
    return { what: "The rating", block: `fn save(m: Model) -> Str = m.rating.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(n) => if n >= 0 && n <= ${max} { Some(Model { rating: n }) } else { None }\n  None => None\n}`, tests: "test restoresWhatWasSaved = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"x\").isNone()" };
  },
  "app.pagination": (t) => {
    const max = num(t.code, /it <= (\d+)/);
    return { what: "The current page", block: `fn save(m: Model) -> Str = m.page.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(n) => if n >= 1 && n <= ${max} { Some(Model { page: n }) } else { None }\n  None => None\n}`, tests: "test restoresWhatWasSaved = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"0\").isNone()" };
  },
  "app.progress-bar": () => ({ what: "The progress", block: "fn save(m: Model) -> Str = m.percent.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(n) => if n >= 0 && n <= 100 { Some(Model { percent: n }) } else { None }\n  None => None\n}", tests: "test restoresWhatWasSaved = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"101\").isNone()" }),
  "app.name-greeter": () => ({ what: "The name", block: "fn save(m: Model) -> Str = m.name\n\nfn restore(s: Str) -> Option[Model] = Some(Model { name: s })", tests: "test restoresTheName = match restore(save(update(init(), SetName(\"Ada\")))) {\n  Some(m) => m.name == \"Ada\"\n  None => false\n}" }),
  "app.text-stats": () => ({ what: "The text", block: "fn save(m: Model) -> Str = m.text\n\nfn restore(s: Str) -> Option[Model] = Some(Model { text: s })", tests: "test restoresTheText = match restore(save(update(init(), Typed(\"abc\")))) {\n  Some(m) => m.text == \"abc\"\n  None => false\n}" }),
  "app.theme-switch": () => ({ what: "The chosen look", block: "fn save(m: Model) -> Str = if m.dark { \"dark\" } else { \"light\" }\n\nfn restore(s: Str) -> Option[Model] =\n  if s == \"dark\" { Some(Model { dark: true }) } else if s == \"light\" { Some(Model { dark: false }) } else { None }", tests: "test restoresTheLook = match restore(save(update(init(), Switch))) {\n  Some(m) => m.dark\n  None => false\n}\n\ntest ignoresNonsense = restore(\"purple\").isNone()" }),
  "app.tip-calculator": () => ({ what: "The chosen tip percentage", block: "fn save(m: Model) -> Str = m.percent.toStr()\n\nfn restore(s: Str) -> Option[Model] = match s.toInt() {\n  Some(p) => if p >= 0 { Some(Model { bill: \"\", percent: p }) } else { None }\n  None => None\n}", tests: "test restoresThePercent = match restore(save(update(init(), Pick(20)))) {\n  Some(m) => m.percent == 20\n  None => false\n}" }),
  "app.shopping-list": () => ({ what: "The list", block: "fn save(m: Model) -> Str = m.items.join(\"\\n\")\n\nfn restore(s: Str) -> Option[Model] =\n  Some(Model { draft: \"\", items: if s.isEmpty() { [] } else { s.split(\"\\n\") } })", tests: "test restoresTheList = match restore(save(update(update(init(), Draft(\"milk\")), Add))) {\n  Some(m) => m.items == [\"milk\"]\n  None => false\n}" }),
  "app.checklist": () => ({ what: "Which tasks are done", block: "fn save(m: Model) -> Str = [m.a, m.b, m.c].map(x => if x { \"1\" } else { \"0\" }).join(\"\")\n\nfn restore(s: Str) -> Option[Model] = {\n  let cs = s.chars()\n  if cs.len() == 3 { Some(Model { a: cs.get(0) == \"1\", b: cs.get(1) == \"1\", c: cs.get(2) == \"1\" }) } else { None }\n}", tests: "test restoresTheTicks = match restore(save(update(init(), ToggleB))) {\n  Some(m) => m.b && !m.a && !m.c\n  None => false\n}\n\ntest ignoresNonsense = restore(\"ab\").isNone()" }),
};
const twoInts = (what) => ({ what, block: "fn save(m: Model) -> Str = m.a.toStr() ++ \",\" ++ m.b.toStr()\n\nfn restore(s: Str) -> Option[Model] = {\n  let parts = s.split(\",\")\n  match parts.first() {\n    Some(x) => match x.toInt() {\n      Some(a) => match parts.drop(1).first() {\n        Some(y) => match y.toInt() {\n          Some(b) => if a >= 0 && b >= 0 { Some(Model { a: a, b: b }) } else { None }\n          None => None\n        }\n        None => None\n      }\n      None => None\n    }\n    None => None\n  }\n}", tests: "test restoresBothNumbers = restore(save(init())).isSome()\n\ntest ignoresNonsense = restore(\"1\").isNone()" });
persistSpecs["app.votes"] = () => twoInts("Both vote counts");
persistSpecs["app.scoreboard"] = () => twoInts("Both scores");
persistSpecs["app.cart-two"] = () => twoInts("Both quantities");

for (const entry of apps) {
  const spec = persistSpecs[entry.id];
  if (!spec) continue;
  const base = entry.make;
  entry.make = (r) => {
    const t = base(r);
    if (!r.chance(0.5)) return t;
    const p = spec(t);
    return {
      ...t,
      name: `${t.name}-saved`,
      desc: `${t.desc} ${p.what} is remembered in the browser and comes back when the page is opened again (a saved value that makes no sense is ignored).`,
      code: `${t.code.replace("\nfn view(", `\n${p.block}\n\nfn view(`).trimEnd()}\n\n${p.tests}\n`,
    };
  };
}
