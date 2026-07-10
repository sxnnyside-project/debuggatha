import type { Evidence, StackSignal } from "../types.js";

/** Accumulates evidence per detected value so repeated signals merge into one StackSignal. */
export class SignalCollector {
  private readonly signals = new Map<string, Evidence[]>();

  add(value: string, evidence: Evidence): void {
    const existing = this.signals.get(value);
    if (existing) {
      existing.push(evidence);
    } else {
      this.signals.set(value, [evidence]);
    }
  }

  has(value: string): boolean {
    return this.signals.has(value);
  }

  toArray(): StackSignal[] {
    return [...this.signals.entries()]
      .map(([value, evidence]) => ({ value, evidence }))
      .sort((a, b) => a.value.localeCompare(b.value));
  }
}
