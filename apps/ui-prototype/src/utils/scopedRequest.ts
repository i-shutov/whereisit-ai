export interface ScopedRequestTicket {
  key: string;
  sequence: number;
}

/** Tracks the latest request for a workspace scope and remembers only successful results. */
export class ScopedRequestGate {
  private sequence = 0;
  private completedKey: string | null = null;
  private pendingTicket: ScopedRequestTicket | null = null;

  begin(key: string): ScopedRequestTicket | null {
    if (this.pendingTicket?.key === key) return null;
    if (this.pendingTicket) this.invalidate(this.pendingTicket);
    if (this.completedKey === key) return null;
    const ticket = { key, sequence: ++this.sequence };
    this.pendingTicket = ticket;
    return ticket;
  }

  isCurrent(ticket: ScopedRequestTicket): boolean {
    return ticket.sequence === this.sequence;
  }

  succeed(ticket: ScopedRequestTicket): boolean {
    if (!this.isCurrent(ticket) || this.pendingTicket !== ticket) return false;
    this.pendingTicket = null;
    this.completedKey = ticket.key;
    return true;
  }

  fail(ticket: ScopedRequestTicket): boolean {
    if (!this.isCurrent(ticket) || this.pendingTicket !== ticket) return false;
    this.pendingTicket = null;
    return true;
  }

  invalidate(ticket?: ScopedRequestTicket): void {
    if (ticket && (this.pendingTicket !== ticket || !this.isCurrent(ticket))) return;
    this.sequence += 1;
    this.pendingTicket = null;
  }
}
