// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class WeatherSocActive implements BaboonGeneratedLatest {
    private readonly _bucket: string;
    private readonly _cold: boolean;

    constructor(bucket: string, cold: boolean) {
        this._bucket = bucket
        this._cold = cold
    }

    public get bucket(): string {
        return this._bucket;
    }
    public get cold(): boolean {
        return this._cold;
    }

    public toJSON(): Record<string, unknown> {
        return {
            bucket: this._bucket,
            cold: this._cold
        };
    }

    public with(overrides: {bucket?: string; cold?: boolean}): WeatherSocActive {
        return new WeatherSocActive(
            'bucket' in overrides ? overrides.bucket! : this._bucket,
            'cold' in overrides ? overrides.cold! : this._cold
        );
    }

    public static fromPlain(obj: {bucket: string; cold: boolean}): WeatherSocActive {
        return new WeatherSocActive(
            obj.bucket,
            obj.cold
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocActive.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocActive.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocActive'
    public baboonTypeIdentifier() {
        return WeatherSocActive.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return WeatherSocActive.BaboonSameInVersions
    }
    public static binCodec(): WeatherSocActive_UEBACodec {
        return WeatherSocActive_UEBACodec.instance
    }
}

export class WeatherSocActive_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocActive, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocActive_UEBACodec.lazyInstance.value) {
          return WeatherSocActive_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.bucket);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeBool(buffer, value.cold);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.bucket);
            BinTools.writeBool(writer, value.cold);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocActive {
        if (this !== WeatherSocActive_UEBACodec .lazyInstance.value) {
            return WeatherSocActive_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const bucket = BinTools.readString(reader);
        const cold = BinTools.readBool(reader);
        return new WeatherSocActive(
            bucket,
            cold,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocActive_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocActive_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocActive'
    public baboonTypeIdentifier() {
        return WeatherSocActive_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocActive_UEBACodec())
    public static get instance(): WeatherSocActive_UEBACodec {
        return WeatherSocActive_UEBACodec.lazyInstance.value
    }
}