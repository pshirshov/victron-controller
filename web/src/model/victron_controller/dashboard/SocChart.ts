// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {SocProjection, SocProjection_UEBACodec} from './SocProjection'
import {SocHistorySample, SocHistorySample_UEBACodec} from './SocHistorySample'

export class SocChart implements BaboonGeneratedLatest {
    private readonly _history: Array<SocHistorySample>;
    private readonly _projection: SocProjection;
    private readonly _now_epoch_ms: bigint;
    private readonly _now_soc_pct: number | undefined;
    private readonly _discharge_target_pct: number | undefined;
    private readonly _charge_target_pct: number | undefined;

    constructor(history: Array<SocHistorySample>, projection: SocProjection, now_epoch_ms: bigint, now_soc_pct: number | undefined, discharge_target_pct: number | undefined, charge_target_pct: number | undefined) {
        this._history = history
        this._projection = projection
        this._now_epoch_ms = now_epoch_ms
        this._now_soc_pct = now_soc_pct
        this._discharge_target_pct = discharge_target_pct
        this._charge_target_pct = charge_target_pct
    }

    public get history(): Array<SocHistorySample> {
        return this._history;
    }
    public get projection(): SocProjection {
        return this._projection;
    }
    public get now_epoch_ms(): bigint {
        return this._now_epoch_ms;
    }
    public get now_soc_pct(): number | undefined {
        return this._now_soc_pct;
    }
    public get discharge_target_pct(): number | undefined {
        return this._discharge_target_pct;
    }
    public get charge_target_pct(): number | undefined {
        return this._charge_target_pct;
    }

    public toJSON(): Record<string, unknown> {
        return {
            history: this._history,
            projection: this._projection,
            now_epoch_ms: this._now_epoch_ms,
            now_soc_pct: this._now_soc_pct !== undefined ? this._now_soc_pct : undefined,
            discharge_target_pct: this._discharge_target_pct !== undefined ? this._discharge_target_pct : undefined,
            charge_target_pct: this._charge_target_pct !== undefined ? this._charge_target_pct : undefined
        };
    }

    public with(overrides: {history?: Array<SocHistorySample>; projection?: SocProjection; now_epoch_ms?: bigint; now_soc_pct?: number | undefined; discharge_target_pct?: number | undefined; charge_target_pct?: number | undefined}): SocChart {
        return new SocChart(
            'history' in overrides ? overrides.history! : this._history,
            'projection' in overrides ? overrides.projection! : this._projection,
            'now_epoch_ms' in overrides ? overrides.now_epoch_ms! : this._now_epoch_ms,
            'now_soc_pct' in overrides ? overrides.now_soc_pct! : this._now_soc_pct,
            'discharge_target_pct' in overrides ? overrides.discharge_target_pct! : this._discharge_target_pct,
            'charge_target_pct' in overrides ? overrides.charge_target_pct! : this._charge_target_pct
        );
    }

    public static fromPlain(obj: {history: Array<SocHistorySample>; projection: SocProjection; now_epoch_ms: bigint; now_soc_pct: number | undefined; discharge_target_pct: number | undefined; charge_target_pct: number | undefined}): SocChart {
        return new SocChart(
            obj.history,
            obj.projection,
            obj.now_epoch_ms,
            obj.now_soc_pct,
            obj.discharge_target_pct,
            obj.charge_target_pct
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return SocChart.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocChart.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocChart'
    public baboonTypeIdentifier() {
        return SocChart.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return SocChart.BaboonSameInVersions
    }
    public static binCodec(): SocChart_UEBACodec {
        return SocChart_UEBACodec.instance
    }
}

export class SocChart_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SocChart, writer: BaboonBinWriter): unknown {
        if (this !== SocChart_UEBACodec.lazyInstance.value) {
          return SocChart_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.history).length);
            for (const item of value.history) {
                SocHistorySample_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                SocProjection_UEBACodec.instance.encode(ctx, value.projection, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.now_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.now_soc_pct === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.now_soc_pct);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.discharge_target_pct === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.discharge_target_pct);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.charge_target_pct === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.charge_target_pct);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, Array.from(value.history).length);
            for (const item of value.history) {
                SocHistorySample_UEBACodec.instance.encode(ctx, item, writer);
            }
            SocProjection_UEBACodec.instance.encode(ctx, value.projection, writer);
            BinTools.writeI64(writer, value.now_epoch_ms);
            if (value.now_soc_pct === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.now_soc_pct);
            }
            if (value.discharge_target_pct === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.discharge_target_pct);
            }
            if (value.charge_target_pct === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.charge_target_pct);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SocChart {
        if (this !== SocChart_UEBACodec .lazyInstance.value) {
            return SocChart_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 5; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const history = Array.from({ length: BinTools.readI32(reader) }, () => SocHistorySample_UEBACodec.instance.decode(ctx, reader));
        const projection = SocProjection_UEBACodec.instance.decode(ctx, reader);
        const now_epoch_ms = BinTools.readI64(reader);
        const now_soc_pct = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const discharge_target_pct = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const charge_target_pct = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        return new SocChart(
            history,
            projection,
            now_epoch_ms,
            now_soc_pct,
            discharge_target_pct,
            charge_target_pct,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return SocChart_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocChart_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocChart'
    public baboonTypeIdentifier() {
        return SocChart_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SocChart_UEBACodec())
    public static get instance(): SocChart_UEBACodec {
        return SocChart_UEBACodec.lazyInstance.value
    }
}