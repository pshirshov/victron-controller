import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum DischargeTime {
    At0200 = "At0200",
    At2300 = "At2300"
}

export const DischargeTime_values: ReadonlyArray<DischargeTime> = [
    DischargeTime.At0200,
    DischargeTime.At2300
] as const;

export function DischargeTime_parse(s: string): DischargeTime {
    const found = DischargeTime_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown DischargeTime variant: " + s);
    }
    return found;
}

export class DischargeTime_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: DischargeTime, writer: BaboonBinWriter): unknown {
        if (this !== DischargeTime_UEBACodec.lazyInstance.value) {
          return DischargeTime_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "At0200": BinTools.writeByte(writer, 0); break;
                case "At2300": BinTools.writeByte(writer, 1); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): DischargeTime {
        if (this !== DischargeTime_UEBACodec .lazyInstance.value) {
            return DischargeTime_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "At0200" as DischargeTime;
                case 1: return "At2300" as DischargeTime;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return DischargeTime_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return DischargeTime_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#DischargeTime'
    public baboonTypeIdentifier() {
        return DischargeTime_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new DischargeTime_UEBACodec())
    public static get instance(): DischargeTime_UEBACodec {
        return DischargeTime_UEBACodec.lazyInstance.value
    }
}