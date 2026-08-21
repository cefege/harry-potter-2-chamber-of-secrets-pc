//===============================================================================
//  #19 Newt Scamander
//===============================================================================

class  WCScamander extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardScamanderTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Scamandersmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardScamanderBigTexture FILE=TEXTURES\menu\Folio\Cards\Scamanderbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Newt Scamander";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=19
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardScamanderTex0'
	textureBig=Texture'WizCardScamanderBigTexture'
	strDescriptionId="WizCard_0027"
}
