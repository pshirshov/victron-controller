// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {DecisionFactor, DecisionFactor_UEBACodec} from './DecisionFactor'

export class Decision implements BaboonGeneratedLatest {
    private readonly _summary: string;
    private readonly _factors: Array<DecisionFactor>;

    constructor(summary: string, factors: Array<DecisionFactor>) {
        this._summary = summary
        this._factors = factors
    }

    public get summary(): string {
        return this._summary;
    }
    public get factors(): Array<DecisionFactor> {
        return this._factors;
    }

    public toJSON(): Record<string, unknown> {
        return {
            summary: this._summary,
            factors: this._factors
        };
    }

    public with(overrides: {summary?: string; factors?: Array<DecisionFactor>}): Decision {
        return new Decision(
            'summary' in overrides ? overrides.summary! : this._summary,
            'factors' in overrides ? overrides.factors! : this._factors
        );
    }

    public static fromPlain(obj: {summary: string; factors: Array<DecisionFactor>}): Decision {
        return new Decision(
            obj.summary,
            obj.factors
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Decision.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Decision.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Decision'
    public baboonTypeIdentifier() {
        return Decision.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Decision.BaboonSameInVersions
    }
    public static binCodec(): Decision_UEBACodec {
        return Decision_UEBACodec.instance
    }
}

export class Decision_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Decision, writer: BaboonBinWriter): unknown {
        if (this !== Decision_UEBACodec.lazyInstance.value) {
          return Decision_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.summary);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.factors).length);
            for (const item of value.factors) {
                DecisionFactor_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.summary);
            BinTools.writeI32(writer, Array.from(value.factors).length);
            for (const item of value.factors) {
                DecisionFactor_UEBACodec.instance.encode(ctx, item, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Decision {
        if (this !== Decision_UEBACodec .lazyInstance.value) {
            return Decision_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const summary = BinTools.readString(reader);
        const factors = Array.from({ length: BinTools.readI32(reader) }, () => DecisionFactor_UEBACodec.instance.decode(ctx, reader));
        return new Decision(
            summary,
            factors,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Decision_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Decision_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Decision'
    public baboonTypeIdentifier() {
        return Decision_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Decision_UEBACodec())
    public static get instance(): Decision_UEBACodec {
        return Decision_UEBACodec.lazyInstance.value
    }
}