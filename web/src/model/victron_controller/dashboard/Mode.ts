// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum Mode {
    Weather = "Weather",
    Forced = "Forced"
}

export const Mode_values: ReadonlyArray<Mode> = [
    Mode.Weather,
    Mode.Forced
] as const;

export function Mode_parse(s: string): Mode {
    const found = Mode_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown Mode variant: " + s);
    }
    return found;
}

export class Mode_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Mode, writer: BaboonBinWriter): unknown {
        if (this !== Mode_UEBACodec.lazyInstance.value) {
          return Mode_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Weather": BinTools.writeByte(writer, 0); break;
                case "Forced": BinTools.writeByte(writer, 1); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Mode {
        if (this !== Mode_UEBACodec .lazyInstance.value) {
            return Mode_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Weather" as Mode;
                case 1: return "Forced" as Mode;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Mode_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Mode_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Mode'
    public baboonTypeIdentifier() {
        return Mode_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Mode_UEBACodec())
    public static get instance(): Mode_UEBACodec {
        return Mode_UEBACodec.lazyInstance.value
    }
}