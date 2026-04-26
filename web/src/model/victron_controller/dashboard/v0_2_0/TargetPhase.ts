// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum TargetPhase {
    Unset = "Unset",
    Pending = "Pending",
    Commanded = "Commanded",
    Confirmed = "Confirmed"
}

export const TargetPhase_values: ReadonlyArray<TargetPhase> = [
    TargetPhase.Unset,
    TargetPhase.Pending,
    TargetPhase.Commanded,
    TargetPhase.Confirmed
] as const;

export function TargetPhase_parse(s: string): TargetPhase {
    const found = TargetPhase_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown TargetPhase variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class TargetPhase_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: TargetPhase, writer: BaboonBinWriter): unknown {
        if (this !== TargetPhase_UEBACodec.lazyInstance.value) {
          return TargetPhase_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Unset": BinTools.writeByte(writer, 0); break;
                case "Pending": BinTools.writeByte(writer, 1); break;
                case "Commanded": BinTools.writeByte(writer, 2); break;
                case "Confirmed": BinTools.writeByte(writer, 3); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): TargetPhase {
        if (this !== TargetPhase_UEBACodec .lazyInstance.value) {
            return TargetPhase_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Unset" as TargetPhase;
                case 1: return "Pending" as TargetPhase;
                case 2: return "Commanded" as TargetPhase;
                case 3: return "Confirmed" as TargetPhase;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return TargetPhase_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TargetPhase_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TargetPhase'
    public baboonTypeIdentifier() {
        return TargetPhase_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new TargetPhase_UEBACodec())
    public static get instance(): TargetPhase_UEBACodec {
        return TargetPhase_UEBACodec.lazyInstance.value
    }
}