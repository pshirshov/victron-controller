// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class WsLogLine implements BaboonGenerated {
    private readonly _at_epoch_ms: bigint;
    private readonly _level: string;
    private readonly _source: string;
    private readonly _message: string;

    constructor(at_epoch_ms: bigint, level: string, source: string, message: string) {
        this._at_epoch_ms = at_epoch_ms
        this._level = level
        this._source = source
        this._message = message
    }

    public get at_epoch_ms(): bigint {
        return this._at_epoch_ms;
    }
    public get level(): string {
        return this._level;
    }
    public get source(): string {
        return this._source;
    }
    public get message(): string {
        return this._message;
    }

    public toJSON(): Record<string, unknown> {
        return {
            at_epoch_ms: this._at_epoch_ms,
            level: this._level,
            source: this._source,
            message: this._message
        };
    }

    public with(overrides: {at_epoch_ms?: bigint; level?: string; source?: string; message?: string}): WsLogLine {
        return new WsLogLine(
            'at_epoch_ms' in overrides ? overrides.at_epoch_ms! : this._at_epoch_ms,
            'level' in overrides ? overrides.level! : this._level,
            'source' in overrides ? overrides.source! : this._source,
            'message' in overrides ? overrides.message! : this._message
        );
    }

    public static fromPlain(obj: {at_epoch_ms: bigint; level: string; source: string; message: string}): WsLogLine {
        return new WsLogLine(
            obj.at_epoch_ms,
            obj.level,
            obj.source,
            obj.message
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return WsLogLine.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsLogLine.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsLogLine'
    public baboonTypeIdentifier() {
        return WsLogLine.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return WsLogLine.BaboonSameInVersions
    }
    public static binCodec(): WsLogLine_UEBACodec {
        return WsLogLine_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class WsLogLine_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WsLogLine, writer: BaboonBinWriter): unknown {
        if (this !== WsLogLine_UEBACodec.lazyInstance.value) {
          return WsLogLine_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.at_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.level);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.source);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.message);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.at_epoch_ms);
            BinTools.writeString(writer, value.level);
            BinTools.writeString(writer, value.source);
            BinTools.writeString(writer, value.message);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WsLogLine {
        if (this !== WsLogLine_UEBACodec .lazyInstance.value) {
            return WsLogLine_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 3; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const at_epoch_ms = BinTools.readI64(reader);
        const level = BinTools.readString(reader);
        const source = BinTools.readString(reader);
        const message = BinTools.readString(reader);
        return new WsLogLine(
            at_epoch_ms,
            level,
            source,
            message,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return WsLogLine_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsLogLine_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsLogLine'
    public baboonTypeIdentifier() {
        return WsLogLine_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WsLogLine_UEBACodec())
    public static get instance(): WsLogLine_UEBACodec {
        return WsLogLine_UEBACodec.lazyInstance.value
    }
}