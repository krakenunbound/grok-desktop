<script lang="ts">
  import { pendingInteraction, resolveInteraction, type GrokQuestion } from "$lib/stores/chat";

  let selected = $state<Record<string, string[]>>({});
  let notes = $state<Record<string, string>>({});
  let feedback = $state("");
  let deciding = $state(false);
  let activeRequestId = "";

  $effect(() => {
    const requestId = $pendingInteraction?.request_id ?? "";
    if (requestId !== activeRequestId) {
      activeRequestId = requestId;
      selected = {};
      notes = {};
      feedback = "";
      deciding = false;
    }
  });

  function key(question: GrokQuestion, index: number): string {
    return question.id || question.question || `Question ${index + 1}`;
  }

  function choiceKey(question: GrokQuestion): string {
    return question.question;
  }

  function toggle(question: GrokQuestion, index: number, label: string): void {
    const id = key(question, index);
    notes[id] = "";
    const current = selected[id] ?? [];
    if (question.multiSelect) {
      selected[id] = current.includes(label)
        ? current.filter((value) => value !== label)
        : [...current, label];
    } else {
      selected[id] = [label];
    }
  }

  function partialAnswers(): Record<string, string> {
    const result: Record<string, string> = {};
    for (const [index, question] of ($pendingInteraction?.questions ?? []).entries()) {
      const answer = selected[key(question, index)]?.[0];
      if (answer) result[choiceKey(question)] = answer;
    }
    return result;
  }

  async function submitQuestions(outcome: "accepted" | "chat_about_this" | "skip_interview") {
    const questions = $pendingInteraction?.questions ?? [];
    if (outcome !== "accepted") {
      await decide({ outcome, partial_answers: partialAnswers() });
      return;
    }
    const answers: Record<string, string[]> = {};
    const annotations: Record<string, { notes: string }> = {};
    for (const [index, question] of questions.entries()) {
      const id = key(question, index);
      const values = selected[id] ?? [];
      const note = notes[id]?.trim();
      if (note) {
        answers[choiceKey(question)] = ["Other"];
        annotations[choiceKey(question)] = { notes: note };
      } else if (values.length) {
        answers[choiceKey(question)] = values;
      }
    }
    if (Object.keys(answers).length !== questions.length) return;
    await decide({
      outcome,
      answers,
      ...(Object.keys(annotations).length ? { annotations } : {}),
    });
  }

  function allAnswered(): boolean {
    return ($pendingInteraction?.questions ?? []).every((question, index) => {
      const id = key(question, index);
      return (selected[id]?.length ?? 0) > 0 || !!notes[id]?.trim();
    });
  }

  async function decide(response: Record<string, unknown>) {
    deciding = true;
    try {
      await resolveInteraction(response);
    } finally {
      deciding = false;
    }
  }
</script>

{#if $pendingInteraction}
  <div class="interaction" role="alertdialog" aria-labelledby="interaction-title">
    {#if $pendingInteraction.kind === "question"}
      <header>
        <div>
          <div class="eyebrow">Grok needs your input</div>
          <strong id="interaction-title">Answer to continue</strong>
        </div>
        <button
          class="quiet"
          type="button"
          disabled={deciding}
          onclick={() => decide({ outcome: "cancelled" })}>Cancel</button
        >
      </header>
      <div class="questions">
        {#each $pendingInteraction.questions as question, index (key(question, index))}
          <fieldset>
            <legend>{question.question}</legend>
            <div class="options">
              {#each question.options as option (option.id || option.label)}
                <button
                  type="button"
                  class:selected={(selected[key(question, index)] ?? []).includes(option.label)}
                  disabled={deciding}
                  onclick={() => toggle(question, index, option.label)}
                >
                  <span>{option.label}</span>
                  {#if option.description}<small>{option.description}</small>{/if}
                  {#if option.preview}<pre>{option.preview}</pre>{/if}
                </button>
              {/each}
            </div>
            <label class="other">
              <span>Something else</span>
              <input
                value={notes[key(question, index)] ?? ""}
                oninput={(event) => {
                  const id = key(question, index);
                  notes[id] = event.currentTarget.value;
                  if (event.currentTarget.value) selected[id] = [];
                }}
                placeholder="Type your answer…"
                disabled={deciding}
              />
            </label>
          </fieldset>
        {/each}
      </div>
      <footer>
        {#if $pendingInteraction.mode === "plan"}
          <button
            type="button"
            disabled={deciding}
            onclick={() => submitQuestions("chat_about_this")}>Chat about this</button
          >
          <button
            type="button"
            disabled={deciding}
            onclick={() => submitQuestions("skip_interview")}>Skip interview</button
          >
        {/if}
        <button
          class="primary"
          type="button"
          disabled={deciding || !allAnswered()}
          onclick={() => submitQuestions("accepted")}>Continue</button
        >
      </footer>
    {:else}
      <header>
        <div>
          <div class="eyebrow">Plan ready</div>
          <strong id="interaction-title">Review Grok’s plan</strong>
        </div>
      </header>
      <pre class="plan">{$pendingInteraction.plan_content ||
          "Grok did not include plan details."}</pre>
      <label class="feedback">
        <span>Changes you want (optional)</span>
        <textarea
          rows="2"
          bind:value={feedback}
          placeholder="Tell Grok what to revise…"
          disabled={deciding}
        ></textarea>
      </label>
      <footer>
        <button
          class="quiet"
          type="button"
          disabled={deciding}
          onclick={() => decide({ outcome: "abandoned" })}>Stop planning</button
        >
        <button
          type="button"
          disabled={deciding || !feedback.trim()}
          onclick={() => decide({ outcome: "cancelled", feedback: feedback.trim() })}
          >Request changes</button
        >
        <button
          class="primary"
          type="button"
          disabled={deciding}
          onclick={() => decide({ outcome: "approved" })}>Approve plan</button
        >
      </footer>
    {/if}
  </div>
{/if}

<style>
  .interaction {
    margin-bottom: 0.65rem;
    padding: 0.85rem;
    border: 1px solid color-mix(in srgb, var(--accent) 65%, var(--border));
    border-radius: 12px;
    background: color-mix(in srgb, var(--surface) 94%, var(--accent) 6%);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3);
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }
  .eyebrow {
    color: var(--accent);
    font-size: 0.68rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  strong {
    display: block;
    margin-top: 0.12rem;
    font-size: 0.92rem;
  }
  .questions {
    display: grid;
    gap: 0.65rem;
    max-height: 42vh;
    overflow: auto;
    margin: 0.7rem 0;
  }
  fieldset {
    min-width: 0;
    margin: 0;
    padding: 0.65rem;
    border: 1px solid var(--border);
    border-radius: 9px;
  }
  legend {
    padding: 0 0.25rem;
    font-size: 0.83rem;
    font-weight: 750;
  }
  .options {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 0.4rem;
  }
  button {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.48rem 0.7rem;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-size: 0.77rem;
    font-weight: 700;
    cursor: pointer;
  }
  .options button {
    text-align: left;
  }
  .options button span,
  .options button small {
    display: block;
  }
  .options button small {
    margin-top: 0.15rem;
    color: var(--muted);
    font-size: 0.69rem;
    font-weight: 500;
  }
  .options button.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--surface-2) 80%, var(--accent) 20%);
  }
  .options pre {
    max-height: 70px;
    overflow: auto;
    margin: 0.4rem 0 0;
    color: var(--muted);
    font-size: 0.68rem;
    white-space: pre-wrap;
  }
  .other,
  .feedback {
    display: grid;
    gap: 0.3rem;
    margin-top: 0.5rem;
    color: var(--muted);
    font-size: 0.7rem;
  }
  input,
  textarea {
    box-sizing: border-box;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 0.48rem 0.55rem;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }
  .plan {
    max-height: 35vh;
    overflow: auto;
    margin: 0.7rem 0;
    padding: 0.65rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
    font:
      0.74rem/1.5 ui-monospace,
      SFMono-Regular,
      Menlo,
      Consolas,
      monospace;
    white-space: pre-wrap;
  }
  footer {
    justify-content: flex-end;
    flex-wrap: wrap;
    margin-top: 0.65rem;
  }
  button.primary {
    border-color: var(--accent-dim);
    background: var(--accent-gradient);
    color: var(--accent-contrast);
  }
  button.quiet {
    color: var(--muted);
  }
  button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
  @media (max-width: 760px) {
    .options {
      grid-template-columns: 1fr;
    }
    footer button {
      flex: 1;
    }
  }
</style>
