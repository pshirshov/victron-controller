// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {SocProjectionSegment, SocProjectionSegment_UEBACodec} from './SocProjectionSegment'

export class SocProjection implements BaboonGeneratedLatest {
    private readonly _segments: Array<SocProjectionSegment>;
    private readonly _net_power_w: number | undefined;
    private readonly _capacity_wh: number | undefined;
    private readonly _charge_rate_w: number | undefined;

    constructor(segments: Array<SocProjectionSegment>, net_power_w: number | undefined, capacity_wh: number | undefined, charge_rate_w: number | undefined) {
        this._segments = segments
        this._net_power_w = net_power_w
        this._capacity_wh = capacity_wh
        this._charge_rate_w = charge_rate_w
    }

    public get segments(): Array<SocProjectionSegment> {
        return this._segments;
    }
    public get net_power_w(): number | undefined {
        return this._net_power_w;
    }
    public get capacity_wh(): number | undefined {
        return this._capacity_wh;
    }
    public get charge_rate_w(): number | undefined {
        return this._charge_rate_w;
    }

    public toJSON(): Record<string, unknown> {
        return {
            segments: this._segments,
            net_power_w: this._net_power_w !== undefined ? this._net_power_w : undefined,
            capacity_wh: this._capacity_wh !== undefined ? this._capacity_wh : undefined,
            charge_rate_w: this._charge_rate_w !== undefined ? this._charge_rate_w : undefined
        };
    }

    public with(overrides: {segments?: Array<SocProjectionSegment>; net_power_w?: number | undefined; capacity_wh?: number | undefined; charge_rate_w?: number | undefined}): SocProjection {
        return new SocProjection(
            'segments' in overrides ? overrides.segments! : this._segments,
            'net_power_w' in overrides ? overrides.net_power_w! : this._net_power_w,
            'capacity_wh' in overrides ? overrides.capacity_wh! : this._capacity_wh,
            'charge_rate_w' in overrides ? overrides.charge_rate_w! : this._charge_rate_w
        );
    }

    public static fromPlain(obj: {segments: Array<SocProjectionSegment>; net_power_w: number | undefined; capacity_wh: number | undefined; charge_rate_w: number | undefined}): SocProjection {
        return new SocProjection(
            obj.segments,
            obj.net_power_w,
            obj.capacity_wh,
            obj.charge_rate_w
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
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
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
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
                BinTools.writeI32(buffer, Array.from(value.segments).length);
            for (const item of value.segments) {
                SocProjectionSegment_UEBACodec.instance.encode(ctx, item, buffer);
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
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.charge_rate_w === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.charge_rate_w);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, Array.from(value.segments).length);
            for (const item of value.segments) {
                SocProjectionSegment_UEBACodec.instance.encode(ctx, item, writer);
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
            if (value.charge_rate_w === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.charge_rate_w);
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
            for (let i = 0; i < 4; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const segments = Array.from({ length: BinTools.readI32(reader) }, () => SocProjectionSegment_UEBACodec.instance.decode(ctx, reader));
        const net_power_w = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const capacity_wh = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const charge_rate_w = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        return new SocProjection(
            segments,
            net_power_w,
            capacity_wh,
            charge_rate_w,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
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