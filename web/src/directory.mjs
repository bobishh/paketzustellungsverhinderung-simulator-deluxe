import { DurableObject } from 'cloudflare:workers';

// Each partition owns only short-lived public invitations, never game state.
export class MeshDirectory extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS meshes (room TEXT PRIMARY KEY, invitation TEXT NOT NULL, expires INTEGER NOT NULL)');
  }
  prune() { this.ctx.storage.sql.exec('DELETE FROM meshes WHERE expires <= ?', Date.now()); }
  list() {
    this.prune();
    return this.ctx.storage.sql.exec('SELECT invitation FROM meshes ORDER BY expires DESC LIMIT 64').toArray().map(row => JSON.parse(row.invitation));
  }
  async publish(invite) {
    this.prune();
    const count = this.ctx.storage.sql.exec('SELECT count(*) AS n FROM meshes').one().n;
    const exists = this.ctx.storage.sql.exec('SELECT room FROM meshes WHERE room = ?', invite.room).toArray().length;
    if (count >= 64 && !exists) return false;
    this.ctx.storage.sql.exec('INSERT INTO meshes VALUES (?, ?, ?) ON CONFLICT(room) DO UPDATE SET invitation=excluded.invitation, expires=excluded.expires', invite.room, JSON.stringify(invite), Date.now()+90_000);
    if (!await this.ctx.storage.getAlarm()) await this.ctx.storage.setAlarm(Date.now()+90_000);
    return true;
  }
  remove(room) { this.ctx.storage.sql.exec('DELETE FROM meshes WHERE room = ?', room); }
  async alarm() {
    this.prune();
    if (this.ctx.storage.sql.exec('SELECT count(*) AS n FROM meshes').one().n) await this.ctx.storage.setAlarm(Date.now()+90_000);
  }
}
