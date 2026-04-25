// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class DecisionFactor implements BaboonGeneratedLatest {
    private readonly _name: string;
    private readonly _value: string;

    constructor(name: string, value: string) {
        this._name = name
        this._value = value
    }

    public get name(): string {
        return this._name;
    }
    public get value(): string {
        return this._value;
    }

    public toJSON(): Record<string, unknown> {
        return {
            name: this._name,
            value: this._value
        };
    }

    public with(overrides: {name?: string; value?: string}): DecisionFactor {
        return new DecisionFactor(
            'name' in overrides ? overrides.name! : this._name,
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {name: string; value: string}): DecisionFactor {
        return new DecisionFactor(
            obj.name,
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return DecisionFactor.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return DecisionFactor.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#DecisionFactor'
    public baboonTypeIdentifier() {
        return DecisionFactor.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return DecisionFactor.BaboonSameInVersions
    }
    public static binCodec(): DecisionFactor_UEBACodec {
        return DecisionFactor_UEBACodec.instance
    }
}

export class DecisionFactor_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: DecisionFactor, writer: BaboonBinWriter): unknown {
        if (this !== DecisionFactor_UEBACodec.lazyInstance.value) {
          return DecisionFactor_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.name);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.value);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.name);
            BinTools.writeString(writer, value.value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): DecisionFactor {
        if (this !== DecisionFactor_UEBACodec .lazyInstance.value) {
            return DecisionFactor_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const name = BinTools.readString(reader);
        const value = BinTools.readString(reader);
        return new DecisionFactor(
            name,
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return DecisionFactor_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return DecisionFactor_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#DecisionFactor'
    public baboonTypeIdentifier() {
        return DecisionFactor_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new DecisionFactor_UEBACodec())
    public static get instance(): DecisionFactor_UEBACodec {
        return DecisionFactor_UEBACodec.lazyInstance.value
    }
}