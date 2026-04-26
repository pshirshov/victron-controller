// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum SocProjectionKind {
    Natural = "Natural",
    Idle = "Idle",
    ScheduledCharge = "ScheduledCharge",
    FullChargePush = "FullChargePush",
    Clamped = "Clamped",
    SolarCharge = "SolarCharge",
    Drain = "Drain",
    ForcedNoExport = "ForcedNoExport",
    PreserveForZappi = "PreserveForZappi",
    BelowExportThreshold = "BelowExportThreshold",
    EveningDischarge = "EveningDischarge",
    BatteryFull = "BatteryFull"
}

export const SocProjectionKind_values: ReadonlyArray<SocProjectionKind> = [
    SocProjectionKind.Natural,
    SocProjectionKind.Idle,
    SocProjectionKind.ScheduledCharge,
    SocProjectionKind.FullChargePush,
    SocProjectionKind.Clamped,
    SocProjectionKind.SolarCharge,
    SocProjectionKind.Drain,
    SocProjectionKind.ForcedNoExport,
    SocProjectionKind.PreserveForZappi,
    SocProjectionKind.BelowExportThreshold,
    SocProjectionKind.EveningDischarge,
    SocProjectionKind.BatteryFull
] as const;

export function SocProjectionKind_parse(s: string): SocProjectionKind {
    const found = SocProjectionKind_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown SocProjectionKind variant: " + s);
    }
    return found;
}

export class SocProjectionKind_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SocProjectionKind, writer: BaboonBinWriter): unknown {
        if (this !== SocProjectionKind_UEBACodec.lazyInstance.value) {
          return SocProjectionKind_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Natural": BinTools.writeByte(writer, 0); break;
                case "Idle": BinTools.writeByte(writer, 1); break;
                case "ScheduledCharge": BinTools.writeByte(writer, 2); break;
                case "FullChargePush": BinTools.writeByte(writer, 3); break;
                case "Clamped": BinTools.writeByte(writer, 4); break;
                case "SolarCharge": BinTools.writeByte(writer, 5); break;
                case "Drain": BinTools.writeByte(writer, 6); break;
                case "ForcedNoExport": BinTools.writeByte(writer, 7); break;
                case "PreserveForZappi": BinTools.writeByte(writer, 8); break;
                case "BelowExportThreshold": BinTools.writeByte(writer, 9); break;
                case "EveningDischarge": BinTools.writeByte(writer, 10); break;
                case "BatteryFull": BinTools.writeByte(writer, 11); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SocProjectionKind {
        if (this !== SocProjectionKind_UEBACodec .lazyInstance.value) {
            return SocProjectionKind_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Natural" as SocProjectionKind;
                case 1: return "Idle" as SocProjectionKind;
                case 2: return "ScheduledCharge" as SocProjectionKind;
                case 3: return "FullChargePush" as SocProjectionKind;
                case 4: return "Clamped" as SocProjectionKind;
                case 5: return "SolarCharge" as SocProjectionKind;
                case 6: return "Drain" as SocProjectionKind;
                case 7: return "ForcedNoExport" as SocProjectionKind;
                case 8: return "PreserveForZappi" as SocProjectionKind;
                case 9: return "BelowExportThreshold" as SocProjectionKind;
                case 10: return "EveningDischarge" as SocProjectionKind;
                case 11: return "BatteryFull" as SocProjectionKind;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return SocProjectionKind_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocProjectionKind_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocProjectionKind'
    public baboonTypeIdentifier() {
        return SocProjectionKind_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SocProjectionKind_UEBACodec())
    public static get instance(): SocProjectionKind_UEBACodec {
        return SocProjectionKind_UEBACodec.lazyInstance.value
    }
}