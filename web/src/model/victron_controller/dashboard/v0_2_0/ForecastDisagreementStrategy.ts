// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum ForecastDisagreementStrategy {
    Max = "Max",
    Min = "Min",
    Mean = "Mean",
    SolcastIfAvailableElseMean = "SolcastIfAvailableElseMean"
}

export const ForecastDisagreementStrategy_values: ReadonlyArray<ForecastDisagreementStrategy> = [
    ForecastDisagreementStrategy.Max,
    ForecastDisagreementStrategy.Min,
    ForecastDisagreementStrategy.Mean,
    ForecastDisagreementStrategy.SolcastIfAvailableElseMean
] as const;

export function ForecastDisagreementStrategy_parse(s: string): ForecastDisagreementStrategy {
    const found = ForecastDisagreementStrategy_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown ForecastDisagreementStrategy variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class ForecastDisagreementStrategy_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ForecastDisagreementStrategy, writer: BaboonBinWriter): unknown {
        if (this !== ForecastDisagreementStrategy_UEBACodec.lazyInstance.value) {
          return ForecastDisagreementStrategy_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Max": BinTools.writeByte(writer, 0); break;
                case "Min": BinTools.writeByte(writer, 1); break;
                case "Mean": BinTools.writeByte(writer, 2); break;
                case "SolcastIfAvailableElseMean": BinTools.writeByte(writer, 3); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ForecastDisagreementStrategy {
        if (this !== ForecastDisagreementStrategy_UEBACodec .lazyInstance.value) {
            return ForecastDisagreementStrategy_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Max" as ForecastDisagreementStrategy;
                case 1: return "Min" as ForecastDisagreementStrategy;
                case 2: return "Mean" as ForecastDisagreementStrategy;
                case 3: return "SolcastIfAvailableElseMean" as ForecastDisagreementStrategy;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ForecastDisagreementStrategy_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ForecastDisagreementStrategy_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ForecastDisagreementStrategy'
    public baboonTypeIdentifier() {
        return ForecastDisagreementStrategy_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ForecastDisagreementStrategy_UEBACodec())
    public static get instance(): ForecastDisagreementStrategy_UEBACodec {
        return ForecastDisagreementStrategy_UEBACodec.lazyInstance.value
    }
}