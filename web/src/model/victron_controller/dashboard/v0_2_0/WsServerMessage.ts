// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {WorldSnapshot, WorldSnapshot_UEBACodec} from './WorldSnapshot'
import {WsLogLine, WsLogLine_UEBACodec} from './WsLogLine'
import {WsPong, WsPong_UEBACodec} from './WsPong'
import {CommandAck, CommandAck_UEBACodec} from './CommandAck'

export type WsServerMessage = Hello | Pong | Snapshot | Log | Ack

export const WsServerMessage = {
    BaboonDomainVersion: '0.2.0',
    BaboonDomainIdentifier: 'victron_controller.dashboard',
    BaboonTypeIdentifier: 'victron_controller.dashboard/:#WsServerMessage',
    BaboonSameInVersions: ["0.2.0"],
    BaboonAdtTypeIdentifier: 'victron_controller.dashboard/:#WsServerMessage',
    binCodec(): WsServerMessage_UEBACodec {
        return WsServerMessage_UEBACodec.instance
    }
} as const

export function isHello(value: WsServerMessage): value is Hello { return value instanceof Hello; }
export function isPong(value: WsServerMessage): value is Pong { return value instanceof Pong; }
export function isSnapshot(value: WsServerMessage): value is Snapshot { return value instanceof Snapshot; }
export function isLog(value: WsServerMessage): value is Log { return value instanceof Log; }
export function isAck(value: WsServerMessage): value is Ack { return value instanceof Ack; }

export class Hello implements BaboonGenerated {
    private readonly _server_version: string;
    private readonly _server_ts_ms: bigint;

    constructor(server_version: string, server_ts_ms: bigint) {
        this._server_version = server_version
        this._server_ts_ms = server_ts_ms
    }

    public get server_version(): string {
        return this._server_version;
    }
    public get server_ts_ms(): bigint {
        return this._server_ts_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            server_version: this._server_version,
            server_ts_ms: this._server_ts_ms
        };
    }

    public with(overrides: {server_version?: string; server_ts_ms?: bigint}): Hello {
        return new Hello(
            'server_version' in overrides ? overrides.server_version! : this._server_version,
            'server_ts_ms' in overrides ? overrides.server_ts_ms! : this._server_ts_ms
        );
    }

    public static fromPlain(obj: {server_version: string; server_ts_ms: bigint}): Hello {
        return new Hello(
            obj.server_version,
            obj.server_ts_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Hello.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Hello.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Hello'
    public baboonTypeIdentifier() {
        return Hello.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Hello.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Hello'
    public baboonAdtTypeIdentifier() {
        return Hello.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Hello_UEBACodec {
        return Hello_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Hello_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Hello, writer: BaboonBinWriter): unknown {
        if (this !== Hello_UEBACodec.lazyInstance.value) {
          return Hello_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.server_version);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.server_ts_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.server_version);
            BinTools.writeI64(writer, value.server_ts_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Hello {
        if (this !== Hello_UEBACodec .lazyInstance.value) {
            return Hello_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const server_version = BinTools.readString(reader);
        const server_ts_ms = BinTools.readI64(reader);
        return new Hello(
            server_version,
            server_ts_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Hello_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Hello_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Hello'
    public baboonTypeIdentifier() {
        return Hello_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Hello'
    public baboonAdtTypeIdentifier() {
        return Hello_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Hello_UEBACodec())
    public static get instance(): Hello_UEBACodec {
        return Hello_UEBACodec.lazyInstance.value
    }
}

export class Pong implements BaboonGenerated {
    private readonly _body: WsPong;

    constructor(body: WsPong) {
        this._body = body
    }

    public get body(): WsPong {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: WsPong}): Pong {
        return new Pong(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: WsPong}): Pong {
        return new Pong(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Pong.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Pong.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Pong'
    public baboonTypeIdentifier() {
        return Pong.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Pong.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Pong'
    public baboonAdtTypeIdentifier() {
        return Pong.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Pong_UEBACodec {
        return Pong_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Pong_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Pong, writer: BaboonBinWriter): unknown {
        if (this !== Pong_UEBACodec.lazyInstance.value) {
          return Pong_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                WsPong_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            WsPong_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Pong {
        if (this !== Pong_UEBACodec .lazyInstance.value) {
            return Pong_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = WsPong_UEBACodec.instance.decode(ctx, reader);
        return new Pong(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Pong_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Pong_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Pong'
    public baboonTypeIdentifier() {
        return Pong_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Pong'
    public baboonAdtTypeIdentifier() {
        return Pong_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Pong_UEBACodec())
    public static get instance(): Pong_UEBACodec {
        return Pong_UEBACodec.lazyInstance.value
    }
}

export class Snapshot implements BaboonGenerated {
    private readonly _body: WorldSnapshot;

    constructor(body: WorldSnapshot) {
        this._body = body
    }

    public get body(): WorldSnapshot {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: WorldSnapshot}): Snapshot {
        return new Snapshot(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: WorldSnapshot}): Snapshot {
        return new Snapshot(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Snapshot.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Snapshot.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Snapshot'
    public baboonTypeIdentifier() {
        return Snapshot.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return Snapshot.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Snapshot'
    public baboonAdtTypeIdentifier() {
        return Snapshot.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Snapshot_UEBACodec {
        return Snapshot_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Snapshot_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Snapshot, writer: BaboonBinWriter): unknown {
        if (this !== Snapshot_UEBACodec.lazyInstance.value) {
          return Snapshot_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                WorldSnapshot_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            WorldSnapshot_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Snapshot {
        if (this !== Snapshot_UEBACodec .lazyInstance.value) {
            return Snapshot_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = WorldSnapshot_UEBACodec.instance.decode(ctx, reader);
        return new Snapshot(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Snapshot_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Snapshot_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Snapshot'
    public baboonTypeIdentifier() {
        return Snapshot_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Snapshot'
    public baboonAdtTypeIdentifier() {
        return Snapshot_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Snapshot_UEBACodec())
    public static get instance(): Snapshot_UEBACodec {
        return Snapshot_UEBACodec.lazyInstance.value
    }
}

export class Log implements BaboonGenerated {
    private readonly _body: WsLogLine;

    constructor(body: WsLogLine) {
        this._body = body
    }

    public get body(): WsLogLine {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: WsLogLine}): Log {
        return new Log(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: WsLogLine}): Log {
        return new Log(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Log.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Log.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Log'
    public baboonTypeIdentifier() {
        return Log.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Log.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Log'
    public baboonAdtTypeIdentifier() {
        return Log.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Log_UEBACodec {
        return Log_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Log_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Log, writer: BaboonBinWriter): unknown {
        if (this !== Log_UEBACodec.lazyInstance.value) {
          return Log_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                WsLogLine_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            WsLogLine_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Log {
        if (this !== Log_UEBACodec .lazyInstance.value) {
            return Log_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = WsLogLine_UEBACodec.instance.decode(ctx, reader);
        return new Log(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Log_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Log_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Log'
    public baboonTypeIdentifier() {
        return Log_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Log'
    public baboonAdtTypeIdentifier() {
        return Log_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Log_UEBACodec())
    public static get instance(): Log_UEBACodec {
        return Log_UEBACodec.lazyInstance.value
    }
}

export class Ack implements BaboonGenerated {
    private readonly _body: CommandAck;

    constructor(body: CommandAck) {
        this._body = body
    }

    public get body(): CommandAck {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: CommandAck}): Ack {
        return new Ack(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: CommandAck}): Ack {
        return new Ack(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Ack.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Ack.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Ack'
    public baboonTypeIdentifier() {
        return Ack.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Ack.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Ack'
    public baboonAdtTypeIdentifier() {
        return Ack.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Ack_UEBACodec {
        return Ack_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Ack_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Ack, writer: BaboonBinWriter): unknown {
        if (this !== Ack_UEBACodec.lazyInstance.value) {
          return Ack_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                CommandAck_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            CommandAck_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Ack {
        if (this !== Ack_UEBACodec .lazyInstance.value) {
            return Ack_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = CommandAck_UEBACodec.instance.decode(ctx, reader);
        return new Ack(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Ack_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Ack_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Ack'
    public baboonTypeIdentifier() {
        return Ack_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsServerMessage]#Ack'
    public baboonAdtTypeIdentifier() {
        return Ack_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Ack_UEBACodec())
    public static get instance(): Ack_UEBACodec {
        return Ack_UEBACodec.lazyInstance.value
    }
}


/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class WsServerMessage_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WsServerMessage, writer: BaboonBinWriter): unknown {
        if (this !== WsServerMessage_UEBACodec.lazyInstance.value) {
          return WsServerMessage_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (value instanceof Hello) {
                BinTools.writeByte(writer, 0);
                Hello_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof Pong) {
                BinTools.writeByte(writer, 1);
                Pong_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof Snapshot) {
                BinTools.writeByte(writer, 2);
                Snapshot_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof Log) {
                BinTools.writeByte(writer, 3);
                Log_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof Ack) {
                BinTools.writeByte(writer, 4);
                Ack_UEBACodec.instance.encode(ctx, value, writer);
            }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WsServerMessage {
        if (this !== WsServerMessage_UEBACodec .lazyInstance.value) {
            return WsServerMessage_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return Hello_UEBACodec.instance.decode(ctx, reader)
                case 1: return Pong_UEBACodec.instance.decode(ctx, reader)
                case 2: return Snapshot_UEBACodec.instance.decode(ctx, reader)
                case 3: return Log_UEBACodec.instance.decode(ctx, reader)
                case 4: return Ack_UEBACodec.instance.decode(ctx, reader)
            default: throw new Error("Unknown ADT branch tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return WsServerMessage_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsServerMessage_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsServerMessage'
    public baboonTypeIdentifier() {
        return WsServerMessage_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/:#WsServerMessage'
    public baboonAdtTypeIdentifier() {
        return WsServerMessage_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WsServerMessage_UEBACodec())
    public static get instance(): WsServerMessage_UEBACodec {
        return WsServerMessage_UEBACodec.lazyInstance.value
    }
}