// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class CoreFactor implements BaboonGeneratedLatest {
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

    public with(overrides: {name?: string; value?: string}): CoreFactor {
        return new CoreFactor(
            'name' in overrides ? overrides.name! : this._name,
            'value' in overrides ? overrides.value! : this._value
        );
    }

    public static fromPlain(obj: {name: string; value: string}): CoreFactor {
        return new CoreFactor(
            obj.name,
            obj.value
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreFactor.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreFactor.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreFactor'
    public baboonTypeIdentifier() {
        return CoreFactor.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return CoreFactor.BaboonSameInVersions
    }
    public static binCodec(): CoreFactor_UEBACodec {
        return CoreFactor_UEBACodec.instance
    }
}

export class CoreFactor_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: CoreFactor, writer: BaboonBinWriter): unknown {
        if (this !== CoreFactor_UEBACodec.lazyInstance.value) {
          return CoreFactor_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
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
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): CoreFactor {
        if (this !== CoreFactor_UEBACodec .lazyInstance.value) {
            return CoreFactor_UEBACodec.lazyInstance.value.decode(ctx, reader)
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
        return new CoreFactor(
            name,
            value,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreFactor_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreFactor_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreFactor'
    public baboonTypeIdentifier() {
        return CoreFactor_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new CoreFactor_UEBACodec())
    public static get instance(): CoreFactor_UEBACodec {
        return CoreFactor_UEBACodec.lazyInstance.value
    }
}