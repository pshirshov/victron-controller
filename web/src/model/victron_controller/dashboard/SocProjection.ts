// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class SocProjection implements BaboonGeneratedLatest {
    private readonly _slope_pct_per_hour: number | undefined;
    private readonly _terminus_epoch_ms: bigint | undefined;
    private readonly _terminus_soc_pct: number | undefined;
    private readonly _net_power_w: number | undefined;
    private readonly _capacity_wh: number | undefined;

    constructor(slope_pct_per_hour: number | undefined, terminus_epoch_ms: bigint | undefined, terminus_soc_pct: number | undefined, net_power_w: number | undefined, capacity_wh: number | undefined) {
        this._slope_pct_per_hour = slope_pct_per_hour
        this._terminus_epoch_ms = terminus_epoch_ms
        this._terminus_soc_pct = terminus_soc_pct
        this._net_power_w = net_power_w
        this._capacity_wh = capacity_wh
    }

    public get slope_pct_per_hour(): number | undefined {
        return this._slope_pct_per_hour;
    }
    public get terminus_epoch_ms(): bigint | undefined {
        return this._terminus_epoch_ms;
    }
    public get terminus_soc_pct(): number | undefined {
        return this._terminus_soc_pct;
    }
    public get net_power_w(): number | undefined {
        return this._net_power_w;
    }
    public get capacity_wh(): number | undefined {
        return this._capacity_wh;
    }

    public toJSON(): Record<string, unknown> {
        return {
            slope_pct_per_hour: this._slope_pct_per_hour !== undefined ? this._slope_pct_per_hour : undefined,
            terminus_epoch_ms: this._terminus_epoch_ms !== undefined ? this._terminus_epoch_ms : undefined,
            terminus_soc_pct: this._terminus_soc_pct !== undefined ? this._terminus_soc_pct : undefined,
            net_power_w: this._net_power_w !== undefined ? this._net_power_w : undefined,
            capacity_wh: this._capacity_wh !== undefined ? this._capacity_wh : undefined
        };
    }

    public with(overrides: {slope_pct_per_hour?: number | undefined; terminus_epoch_ms?: bigint | undefined; terminus_soc_pct?: number | undefined; net_power_w?: number | undefined; capacity_wh?: number | undefined}): SocProjection {
        return new SocProjection(
            'slope_pct_per_hour' in overrides ? overrides.slope_pct_per_hour! : this._slope_pct_per_hour,
            'terminus_epoch_ms' in overrides ? overrides.terminus_epoch_ms! : this._terminus_epoch_ms,
            'terminus_soc_pct' in overrides ? overrides.terminus_soc_pct! : this._terminus_soc_pct,
            'net_power_w' in overrides ? overrides.net_power_w! : this._net_power_w,
            'capacity_wh' in overrides ? overrides.capacity_wh! : this._capacity_wh
        );
    }

    public static fromPlain(obj: {slope_pct_per_hour: number | undefined; terminus_epoch_ms: bigint | undefined; terminus_soc_pct: number | undefined; net_power_w: number | undefined; capacity_wh: number | undefined}): SocProjection {
        return new SocProjection(
            obj.slope_pct_per_hour,
            obj.terminus_epoch_ms,
            obj.terminus_soc_pct,
            obj.net_power_w,
            obj.capacity_wh
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SocProjection.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocProjection.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocProjection'
    public baboonTypeIdentifier() {
        return SocProjection.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return SocProjection.BaboonSameInVersions
    }
    public static binCodec(): SocProjection_UEBACodec {
        return SocProjection_UEBACodec.instance
    }
}

export class SocProjection_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SocProjection, writer: BaboonBinWriter): unknown {
        if (this !== SocProjection_UEBACodec.lazyInstance.value) {
          return SocProjection_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.slope_pct_per_hour === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.slope_pct_per_hour);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.terminus_epoch_ms === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeI64(buffer, value.terminus_epoch_ms);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.terminus_soc_pct === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.terminus_soc_pct);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.net_power_w === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.net_power_w);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.capacity_wh === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.capacity_wh);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.slope_pct_per_hour === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.slope_pct_per_hour);
            }
            if (value.terminus_epoch_ms === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeI64(writer, value.terminus_epoch_ms);
            }
            if (value.terminus_soc_pct === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.terminus_soc_pct);
            }
            if (value.net_power_w === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.net_power_w);
            }
            if (value.capacity_wh === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.capacity_wh);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SocProjection {
        if (this !== SocProjection_UEBACodec .lazyInstance.value) {
            return SocProjection_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 5; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const slope_pct_per_hour = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const terminus_epoch_ms = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readI64(reader));
        const terminus_soc_pct = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const net_power_w = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const capacity_wh = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        return new SocProjection(
            slope_pct_per_hour,
            terminus_epoch_ms,
            terminus_soc_pct,
            net_power_w,
            capacity_wh,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SocProjection_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocProjection_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocProjection'
    public baboonTypeIdentifier() {
        return SocProjection_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SocProjection_UEBACodec())
    public static get instance(): SocProjection_UEBACodec {
        return SocProjection_UEBACodec.lazyInstance.value
    }
}