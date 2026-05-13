// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class HeatingCurveBucket implements BaboonGeneratedLatest {
    private readonly _outdoor_max_c: number;
    private readonly _water_target_c: number;

    constructor(outdoor_max_c: number, water_target_c: number) {
        this._outdoor_max_c = outdoor_max_c
        this._water_target_c = water_target_c
    }

    public get outdoor_max_c(): number {
        return this._outdoor_max_c;
    }
    public get water_target_c(): number {
        return this._water_target_c;
    }

    public toJSON(): Record<string, unknown> {
        return {
            outdoor_max_c: this._outdoor_max_c,
            water_target_c: this._water_target_c
        };
    }

    public with(overrides: {outdoor_max_c?: number; water_target_c?: number}): HeatingCurveBucket {
        return new HeatingCurveBucket(
            'outdoor_max_c' in overrides ? overrides.outdoor_max_c! : this._outdoor_max_c,
            'water_target_c' in overrides ? overrides.water_target_c! : this._water_target_c
        );
    }

    public static fromPlain(obj: {outdoor_max_c: number; water_target_c: number}): HeatingCurveBucket {
        return new HeatingCurveBucket(
            obj.outdoor_max_c,
            obj.water_target_c
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return HeatingCurveBucket.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return HeatingCurveBucket.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#HeatingCurveBucket'
    public baboonTypeIdentifier() {
        return HeatingCurveBucket.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return HeatingCurveBucket.BaboonSameInVersions
    }
    public static binCodec(): HeatingCurveBucket_UEBACodec {
        return HeatingCurveBucket_UEBACodec.instance
    }
}

export class HeatingCurveBucket_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: HeatingCurveBucket, writer: BaboonBinWriter): unknown {
        if (this !== HeatingCurveBucket_UEBACodec.lazyInstance.value) {
          return HeatingCurveBucket_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeF64(buffer, value.outdoor_max_c);
            BinTools.writeF64(buffer, value.water_target_c);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.outdoor_max_c);
            BinTools.writeF64(writer, value.water_target_c);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): HeatingCurveBucket {
        if (this !== HeatingCurveBucket_UEBACodec .lazyInstance.value) {
            return HeatingCurveBucket_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const outdoor_max_c = BinTools.readF64(reader);
        const water_target_c = BinTools.readF64(reader);
        return new HeatingCurveBucket(
            outdoor_max_c,
            water_target_c,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return HeatingCurveBucket_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return HeatingCurveBucket_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#HeatingCurveBucket'
    public baboonTypeIdentifier() {
        return HeatingCurveBucket_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new HeatingCurveBucket_UEBACodec())
    public static get instance(): HeatingCurveBucket_UEBACodec {
        return HeatingCurveBucket_UEBACodec.lazyInstance.value
    }
}