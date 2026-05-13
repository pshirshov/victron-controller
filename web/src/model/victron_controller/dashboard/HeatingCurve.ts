// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {HeatingCurveBucket, HeatingCurveBucket_UEBACodec} from './HeatingCurveBucket'

export class HeatingCurve implements BaboonGeneratedLatest {
    private readonly _row_0: HeatingCurveBucket;
    private readonly _row_1: HeatingCurveBucket;
    private readonly _row_2: HeatingCurveBucket;
    private readonly _row_3: HeatingCurveBucket;
    private readonly _row_4: HeatingCurveBucket;

    constructor(row_0: HeatingCurveBucket, row_1: HeatingCurveBucket, row_2: HeatingCurveBucket, row_3: HeatingCurveBucket, row_4: HeatingCurveBucket) {
        this._row_0 = row_0
        this._row_1 = row_1
        this._row_2 = row_2
        this._row_3 = row_3
        this._row_4 = row_4
    }

    public get row_0(): HeatingCurveBucket {
        return this._row_0;
    }
    public get row_1(): HeatingCurveBucket {
        return this._row_1;
    }
    public get row_2(): HeatingCurveBucket {
        return this._row_2;
    }
    public get row_3(): HeatingCurveBucket {
        return this._row_3;
    }
    public get row_4(): HeatingCurveBucket {
        return this._row_4;
    }

    public toJSON(): Record<string, unknown> {
        return {
            row_0: this._row_0,
            row_1: this._row_1,
            row_2: this._row_2,
            row_3: this._row_3,
            row_4: this._row_4
        };
    }

    public with(overrides: {row_0?: HeatingCurveBucket; row_1?: HeatingCurveBucket; row_2?: HeatingCurveBucket; row_3?: HeatingCurveBucket; row_4?: HeatingCurveBucket}): HeatingCurve {
        return new HeatingCurve(
            'row_0' in overrides ? overrides.row_0! : this._row_0,
            'row_1' in overrides ? overrides.row_1! : this._row_1,
            'row_2' in overrides ? overrides.row_2! : this._row_2,
            'row_3' in overrides ? overrides.row_3! : this._row_3,
            'row_4' in overrides ? overrides.row_4! : this._row_4
        );
    }

    public static fromPlain(obj: {row_0: HeatingCurveBucket; row_1: HeatingCurveBucket; row_2: HeatingCurveBucket; row_3: HeatingCurveBucket; row_4: HeatingCurveBucket}): HeatingCurve {
        return new HeatingCurve(
            obj.row_0,
            obj.row_1,
            obj.row_2,
            obj.row_3,
            obj.row_4
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return HeatingCurve.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return HeatingCurve.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#HeatingCurve'
    public baboonTypeIdentifier() {
        return HeatingCurve.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return HeatingCurve.BaboonSameInVersions
    }
    public static binCodec(): HeatingCurve_UEBACodec {
        return HeatingCurve_UEBACodec.instance
    }
}

export class HeatingCurve_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: HeatingCurve, writer: BaboonBinWriter): unknown {
        if (this !== HeatingCurve_UEBACodec.lazyInstance.value) {
          return HeatingCurve_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_0, buffer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_1, buffer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_2, buffer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_3, buffer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_4, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_0, writer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_1, writer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_2, writer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_3, writer);
            HeatingCurveBucket_UEBACodec.instance.encode(ctx, value.row_4, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): HeatingCurve {
        if (this !== HeatingCurve_UEBACodec .lazyInstance.value) {
            return HeatingCurve_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const row_0 = HeatingCurveBucket_UEBACodec.instance.decode(ctx, reader);
        const row_1 = HeatingCurveBucket_UEBACodec.instance.decode(ctx, reader);
        const row_2 = HeatingCurveBucket_UEBACodec.instance.decode(ctx, reader);
        const row_3 = HeatingCurveBucket_UEBACodec.instance.decode(ctx, reader);
        const row_4 = HeatingCurveBucket_UEBACodec.instance.decode(ctx, reader);
        return new HeatingCurve(
            row_0,
            row_1,
            row_2,
            row_3,
            row_4,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return HeatingCurve_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return HeatingCurve_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#HeatingCurve'
    public baboonTypeIdentifier() {
        return HeatingCurve_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new HeatingCurve_UEBACodec())
    public static get instance(): HeatingCurve_UEBACodec {
        return HeatingCurve_UEBACodec.lazyInstance.value
    }
}