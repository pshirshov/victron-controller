// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class WsPong implements BaboonGeneratedLatest {
    private readonly _nonce: string;
    private readonly _client_ts_ms: bigint;
    private readonly _server_ts_ms: bigint;

    constructor(nonce: string, client_ts_ms: bigint, server_ts_ms: bigint) {
        this._nonce = nonce
        this._client_ts_ms = client_ts_ms
        this._server_ts_ms = server_ts_ms
    }

    public get nonce(): string {
        return this._nonce;
    }
    public get client_ts_ms(): bigint {
        return this._client_ts_ms;
    }
    public get server_ts_ms(): bigint {
        return this._server_ts_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            nonce: this._nonce,
            client_ts_ms: this._client_ts_ms,
            server_ts_ms: this._server_ts_ms
        };
    }

    public with(overrides: {nonce?: string; client_ts_ms?: bigint; server_ts_ms?: bigint}): WsPong {
        return new WsPong(
            'nonce' in overrides ? overrides.nonce! : this._nonce,
            'client_ts_ms' in overrides ? overrides.client_ts_ms! : this._client_ts_ms,
            'server_ts_ms' in overrides ? overrides.server_ts_ms! : this._server_ts_ms
        );
    }

    public static fromPlain(obj: {nonce: string; client_ts_ms: bigint; server_ts_ms: bigint}): WsPong {
        return new WsPong(
            obj.nonce,
            obj.client_ts_ms,
            obj.server_ts_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return WsPong.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsPong.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsPong'
    public baboonTypeIdentifier() {
        return WsPong.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0"]
    public baboonSameInVersions() {
        return WsPong.BaboonSameInVersions
    }
    public static binCodec(): WsPong_UEBACodec {
        return WsPong_UEBACodec.instance
    }
}

export class WsPong_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WsPong, writer: BaboonBinWriter): unknown {
        if (this !== WsPong_UEBACodec.lazyInstance.value) {
          return WsPong_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
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
            BinTools.writeI64(buffer, value.server_ts_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.nonce);
            BinTools.writeI64(writer, value.client_ts_ms);
            BinTools.writeI64(writer, value.server_ts_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WsPong {
        if (this !== WsPong_UEBACodec .lazyInstance.value) {
            return WsPong_UEBACodec.lazyInstance.value.decode(ctx, reader)
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
        const server_ts_ms = BinTools.readI64(reader);
        return new WsPong(
            nonce,
            client_ts_ms,
            server_ts_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return WsPong_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsPong_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsPong'
    public baboonTypeIdentifier() {
        return WsPong_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WsPong_UEBACodec())
    public static get instance(): WsPong_UEBACodec {
        return WsPong_UEBACodec.lazyInstance.value
    }
}