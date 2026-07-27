// app.js — Tutor client

document.addEventListener("DOMContentLoaded", () => {
  const els = {
    greeting: document.getElementById("greeting"),
    subtitle: document.getElementById("subtitle"),
    startBtn: document.getElementById("startBtn"),
    topic: document.getElementById("topic"),
    skillLevel: document.getElementById("skillLevel"),
    grammarFocus: document.getElementById("grammarFocus"),
    conversation: document.getElementById("conversation"),
    messageForm: document.getElementById("messageForm"),
    userInput: document.getElementById("userInput"),
    emptyState: document.getElementById("emptyState"),
    translateInput: document.getElementById("translateInput"),
    translateOutput: document.getElementById("translateOutput"),
    translateBtn: document.getElementById("translateBtn"),
    translateInputLabel: document.getElementById("translateInputLabel"),
    translateOutputLabel: document.getElementById("translateOutputLabel"),
    modelBadge: document.getElementById("modelBadge"),
  };

  // Conversation state — history is held client-side and replayed to the model.
  const session = {
    active: false,
    topic: "",
    grammar: "",
    skillLevel: "",
    messages: [], // [{ role: 'user' | 'assistant', content }]
    streaming: false,
    translating: false,
  };

  loadGrammarTopics();
  loadGreeting();

  els.startBtn.addEventListener("click", startConversation);
  els.messageForm.addEventListener("submit", onSend);

  // Enter to send, Shift+Enter for newline.
  els.userInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      els.messageForm.requestSubmit();
    }
  });
  els.userInput.addEventListener("input", autoGrow);

  els.translateBtn.addEventListener("click", translate);

  // Enter to translate, Shift+Enter for newline.
  els.translateInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      translate();
    }
  });

  // -------------------------------------------------------------------------
  // Startup data
  // -------------------------------------------------------------------------

  async function loadGrammarTopics() {
    try {
      const res = await fetch("/api/grammar");
      const data = await res.json();
      const topics = (data && data.topics) || [];
      fillGrammar(topics.length ? topics : fallbackTopics());
    } catch (err) {
      console.error("grammar load failed", err);
      fillGrammar(fallbackTopics());
    }
  }

  function fallbackTopics() {
    return ["Présent", "Passé Composé", "Imparfait", "Futur Simple"];
  }

  function fillGrammar(topics) {
    while (els.grammarFocus.options.length > 1) els.grammarFocus.remove(1);
    topics.forEach((t) => {
      const opt = document.createElement("option");
      opt.value = t;
      opt.textContent = t;
      els.grammarFocus.appendChild(opt);
    });
  }

  async function loadGreeting() {
    try {
      const res = await fetch("/api/config");
      const data = await res.json();
      const cfg = (data && data.config) || {};
      const tutor = cfg.TUTOR || "Amélie";
      const user = cfg.USER || "";
      els.greeting.textContent = user ? `Bonjour ${user}` : "Bonjour";
      els.subtitle.textContent = `Je suis ${tutor}, votre tutrice de ${cfg.LEARNING_LANGUAGE || "français"}.`;
      document.querySelector(".avatar").textContent = tutor
        .charAt(0)
        .toUpperCase();
      applyTranslatorLabels(cfg.EXPLANATION_LANGUAGE, cfg.LEARNING_LANGUAGE);
      if (cfg.model) els.modelBadge.textContent = cfg.model;
    } catch (err) {
      console.error("config load failed", err);
      els.greeting.textContent = "Bonjour";
      els.subtitle.textContent = "Bienvenue dans votre tutorat de français.";
    }
  }

  // Set the translator field labels/placeholders from the configured languages.
  function applyTranslatorLabels(explanation, learning) {
    const from = explanation || "Allemand";
    const to = learning || "Français";
    els.translateInputLabel.textContent = from;
    els.translateOutputLabel.textContent = to;
    els.translateInput.placeholder = `Écrivez en ${from.toLowerCase()}…`;
  }

  // -------------------------------------------------------------------------
  // Conversation flow
  // -------------------------------------------------------------------------

  function startConversation() {
    const topic = els.topic.value.trim();
    const skillLevel = els.skillLevel.value;
    const grammar = els.grammarFocus.value;

    if (!topic || !skillLevel || !grammar) {
      flashMissing();
      return;
    }

    session.active = true;
    session.topic = topic;
    session.grammar = grammar;
    session.skillLevel = skillLevel;
    session.messages = [];

    els.conversation.innerHTML = "";
    if (els.emptyState) els.emptyState.style.display = "none";
    els.messageForm.classList.remove("disabled");
    els.userInput.disabled = false;
    els.startBtn.textContent = "Recommencer";

    // Ask the tutor for the opening turn (FIRST TURN PROTOCOL) with empty history.
    streamTutorReply();
  }

  async function onSend(e) {
    e.preventDefault();
    if (!session.active || session.streaming) return;

    const text = els.userInput.value.trim();
    if (!text) return;

    session.messages.push({ role: "user", content: text });
    addBubble("user", text);
    els.userInput.value = "";
    autoGrow();

    await streamTutorReply();
  }

  // Stream one assistant reply from /api/chat into a live-growing bubble.
  async function streamTutorReply() {
    session.streaming = true;
    setBusy(true);

    const bubble = addBubble("tutor", "");
    const body = bubble.querySelector(".bubble-body");
    showTyping(body);

    let acc = "";
    try {
      const res = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          topic: session.topic,
          grammar: session.grammar,
          skillLevel: session.skillLevel,
          messages: session.messages,
        }),
      });

      if (!res.ok || !res.body) throw new Error(`HTTP ${res.status}`);

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let sseBuf = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        sseBuf += decoder.decode(value, { stream: true });

        // Parse SSE events separated by a blank line.
        let sep;
        while ((sep = sseBuf.indexOf("\n\n")) !== -1) {
          const raw = sseBuf.slice(0, sep);
          sseBuf = sseBuf.slice(sep + 2);
          const evt = parseSse(raw);
          if (evt.event === "error") {
            acc += `\n⚠️ ${evt.data}`;
          } else if (evt.event === "done") {
            // handled after loop
          } else if (evt.data) {
            // Token arrives as JSON {"t":"..."} so whitespace is preserved.
            try {
              acc += JSON.parse(evt.data).t;
            } catch {
              acc += evt.data;
            }
          }
          body.textContent = acc;
          scrollDown();
        }
      }
    } catch (err) {
      console.error("chat stream failed", err);
      if (!acc)
        acc = "⚠️ Désolé, je n'ai pas pu répondre. Ollama est-il démarré ?";
      body.textContent = acc;
    } finally {
      if (!acc.trim()) {
        body.textContent = "…";
      } else {
        // Streaming used textContent (safe for partial tokens); now that the
        // reply is complete, render the Markdown subset so **bold** etc. show.
        body.innerHTML = renderMarkdown(acc);
      }
      session.messages.push({ role: "assistant", content: acc });
      session.streaming = false;
      setBusy(false);
      scrollDown();
      saveDialogue();
    }
  }

  // SSE frame -> { event, data }. `data:` lines are concatenated with newlines.
  function parseSse(raw) {
    let event = "message";
    const dataLines = [];
    for (const line of raw.split("\n")) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (line.startsWith("data:"))
        dataLines.push(line.slice(5).replace(/^ /, ""));
    }
    return { event, data: dataLines.join("\n") };
  }

  async function saveDialogue() {
    if (!session.messages.length) return;
    try {
      await fetch("/api/dialogue", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          topic: session.topic,
          grammar: session.grammar,
          skillLevel: session.skillLevel,
          messages: session.messages,
        }),
      });
    } catch (err) {
      console.error("dialogue save failed", err);
    }
  }

  // -------------------------------------------------------------------------
  // Translator
  // -------------------------------------------------------------------------

  // Translate the upper field (explanation language) into the lower field
  // (learning language) via /api/translate. Streams the reply as SSE, like chat.
  async function translate() {
    if (session.translating) return;

    const text = els.translateInput.value.trim();
    if (!text) return;

    session.translating = true;
    els.translateBtn.disabled = true;
    els.translateOutput.innerHTML = "";

    let acc = "";
    try {
      const res = await fetch("/api/translate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text }),
      });

      if (!res.ok || !res.body) throw new Error(`HTTP ${res.status}`);

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let sseBuf = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        sseBuf += decoder.decode(value, { stream: true });

        let sep;
        while ((sep = sseBuf.indexOf("\n\n")) !== -1) {
          const raw = sseBuf.slice(0, sep);
          sseBuf = sseBuf.slice(sep + 2);
          const evt = parseSse(raw);
          if (evt.event === "error") {
            acc += `\n⚠️ ${evt.data}`;
          } else if (evt.event === "done") {
            // handled after loop
          } else if (evt.data) {
            try {
              acc += JSON.parse(evt.data).t;
            } catch {
              acc += evt.data;
            }
          }
          els.translateOutput.innerHTML = renderMarkdown(acc);
        }
      }
    } catch (err) {
      console.error("translate failed", err);
      if (!acc) acc = "⚠️ Traduction impossible. Ollama est-il démarré ?";
      els.translateOutput.innerHTML = renderMarkdown(acc);
    } finally {
      if (!acc.trim()) els.translateOutput.textContent = "…";
      session.translating = false;
      els.translateBtn.disabled = false;
    }
  }

  // Render a small subset of Markdown (**bold**, *italic*, `code`) to safe HTML.
  // HTML is escaped first so model output can never inject markup.
  function renderMarkdown(text) {
    const esc = text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
    return esc
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/(^|[^*])\*([^*]+)\*/g, "$1<em>$2</em>")
      .replace(/\n/g, "<br>");
  }

  // -------------------------------------------------------------------------
  // DOM helpers
  // -------------------------------------------------------------------------

  function addBubble(kind, text) {
    const row = document.createElement("div");
    row.className = `msg msg-${kind}`;
    row.innerHTML = `<div class="bubble"><div class="bubble-body"></div></div>`;
    row.querySelector(".bubble-body").textContent = text;
    els.conversation.appendChild(row);
    scrollDown();
    return row;
  }

  function showTyping(bodyEl) {
    bodyEl.innerHTML =
      '<span class="typing"><span></span><span></span><span></span></span>';
  }

  function setBusy(busy) {
    els.userInput.disabled = busy;
    els.messageForm.querySelector("button").disabled = busy;
    if (!busy) els.userInput.focus();
  }

  function scrollDown() {
    els.conversation.scrollTop = els.conversation.scrollHeight;
  }

  function autoGrow() {
    els.userInput.style.height = "auto";
    els.userInput.style.height =
      Math.min(els.userInput.scrollHeight, 160) + "px";
  }

  function flashMissing() {
    [els.topic, els.skillLevel, els.grammarFocus].forEach((el) => {
      if (!el.value) {
        el.classList.add("field-error");
        setTimeout(() => el.classList.remove("field-error"), 1200);
      }
    });
  }
});
