// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class WsPing implements BaboonGeneratedLatest {
    private readonly _nonce: string;
    private readonly _client_ts_ms: bigint;

    constructor(nonce: string, client_ts_ms: bigint) {
        this._nonce = nonce
        this._client_ts_ms = client_ts_ms
    }

    public get nonce(): string {
        return this._nonce;
    }
    public get client_ts_ms(): bigint {
        return this._client_ts_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            nonce: this._nonce,
            client_ts_ms: this._client_ts_ms
        };
    }

    public with(overrides: {nonce?: string; client_ts_ms?: bigint}): WsPing {
        return new WsPing(
            'nonce' in overrides ? overrides.nonce! : this._nonce,
            'client_ts_ms' in overrides ? overrides.client_ts_ms! : this._client_ts_ms
        );
    }

    public static fromPlain(obj: {nonce: string; client_ts_ms: bigint}): WsPing {
        return new WsPing(
            obj.nonce,
            obj.client_ts_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WsPing.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsPing.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsPing'
    public baboonTypeIdentifier() {
        return WsPing.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return WsPing.BaboonSameInVersions
    }
    public static binCodec(): WsPing_UEBACodec {
        return WsPing_UEBACodec.instance
    }
}

export class WsPing_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WsPing, writer: BaboonBinWriter): unknown {
        if (this !== WsPing_UEBACodec.lazyInstance.value) {
          return WsPing_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.nonce);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.client_ts_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.nonce);
            BinTools.writeI64(writer, value.client_ts_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WsPing {
        if (this !== WsPing_UEBACodec .lazyInstance.value) {
            return WsPing_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const nonce = BinTools.readString(reader);
        const client_ts_ms = BinTools.readI64(reader);
        return new WsPing(
            nonce,
            client_ts_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WsPing_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsPing_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsPing'
    public baboonTypeIdentifier() {
        return WsPing_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WsPing_UEBACodec())
    public static get instance(): WsPing_UEBACodec {
        return WsPing_UEBACodec.lazyInstance.value
    }
}